//! The numeric half of resolution: how large a box is, before anything is
//! drawn.
//!
//! Nothing here builds a rectangle. Every function maps sizes to sizes, which
//! is what lets the rules of `docs/view-model.md`'s Sizing section be checked
//! on their own — and what keeps measuring a subtree distinct from laying it
//! out. The assembly that consumes these numbers lives in
//! [`resolve`](super::resolve).

use crate::text::{PrintableLines, PrintableText};
use crate::{BlockStyle, Length, Sides, View};

use super::geometry::Size;

/// Returns the smaller of two optional bounds, or whichever one exists.
pub(super) fn tighter(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (bound, None) | (None, bound) => bound,
    }
}

/// Applies the clamp the sizing model defines: a base size, capped by the
/// tighter of the maximum and the area, then floored — the floor wins.
pub(super) fn clamp_size(base: usize, cap: Option<usize>, floor: usize) -> usize {
    cap.map_or(base, |cap| base.min(cap)).max(floor)
}

/// One axis of a box, as its style states it.
///
/// The style fixes the frame and the bounds; the content supplies the extents
/// they are clamped against. Both axes use the same rule — they differ only
/// in what they pass as those extents, which is where the model's asymmetry
/// lives rather than in two separate formulas.
#[derive(Debug, Clone, Copy)]
pub(super) struct Axis {
    /// Enabled border edges plus padding on this axis, after degradation.
    pub frame: usize,
    pub length: Option<Length>,
    pub min: Option<u16>,
    pub max: Option<u16>,
}

impl Axis {
    /// The outer size this axis resolves to under `area`.
    ///
    /// `intrinsic` is the content's extent when nothing bounds it, and
    /// `min_content` the extent below which the content cannot go. The width
    /// axis passes the widest unsplittable grapheme as `min_content`; the
    /// height axis passes zero, because a row can simply be absent.
    pub fn used(&self, area: Option<usize>, intrinsic: usize, min_content: usize) -> usize {
        let base = match self.length {
            Some(Length::Cells(cells)) => usize::from(cells),
            // A Fill length needs an area to divide; with none it contributes
            // the intrinsic size.
            Some(Length::Fill(_)) => area.unwrap_or(self.frame + intrinsic),
            None => self.frame + intrinsic,
        };
        clamp_size(
            base,
            tighter(self.max.map(usize::from), area),
            self.min
                .map_or(0, usize::from)
                .max(self.frame + min_content),
        )
    }

    /// The bound the content is laid out under, before its own extent is
    /// known: the tightest of a stated size, the maximum, and the area.
    pub fn content_bound(&self, area: Option<usize>) -> Option<usize> {
        let stated = match self.length {
            Some(Length::Cells(cells)) => Some(usize::from(cells)),
            Some(Length::Fill(_)) => area,
            None => None,
        };
        tighter(stated, tighter(self.max.map(usize::from), area))
            .map(|bound| bound.saturating_sub(self.frame))
    }
}

/// How a child claims space on its container's main axis.
///
/// The three kinds are the distribution rule's three cases, and their order in
/// this enum is also the order a deficit shrinks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// A weighted share of whatever the other children leave.
    Fill(u16),
    /// No stated length: the intrinsic size.
    Auto,
    /// A stated size in cells.
    Cells,
}

/// One child's claim on its container's main axis.
#[derive(Debug, Clone, Copy)]
pub(super) struct Claim {
    pub kind: Kind,
    /// The size the child asks for. A `Fill` child's is decided by
    /// [`distribute`], so its value here is unused.
    pub demand: usize,
    /// The size below which this child cannot be shrunk.
    pub floor: usize,
}

/// Divides `area` among the children that claim it.
///
/// Stated and intrinsic children take their demand; `Fill` children divide
/// what remains in proportion to their weights. When the claims exceed the
/// area, they shrink in the order `Fill`, then auto, then `Cells` —
/// proportionally to their current size within each group, each floored at its
/// own floor, and a floor that binds freezes that child so the shortfall falls
/// on the rest. That freeze loop is an iteration over numbers, settled here
/// before any child is assembled.
///
/// The returned sizes sum to `area` unless a floor prevented it, in which case
/// the container resolves larger than its area and the degenerate safety net
/// is what finally bounds it.
pub(super) fn distribute(area: usize, claims: &[Claim]) -> Vec<usize> {
    let stated: usize = claims
        .iter()
        .filter(|claim| !matches!(claim.kind, Kind::Fill(_)))
        .map(|claim| claim.demand)
        .sum();
    let weights: usize = claims.iter().map(|claim| claim.weight()).sum();
    let remaining = area.saturating_sub(stated);

    // Shares are cut from a running prefix of the remainder, so they sum to it
    // exactly: three equal weights over ten cells are 3, 3, 4, not 3, 3, 3
    // with a cell lost to rounding.
    let mut assigned = Vec::with_capacity(claims.len());
    let (mut weighted, mut given) = (0, 0);
    for claim in claims {
        assigned.push(match claim.kind {
            Kind::Fill(_) if weights > 0 => {
                weighted += claim.weight();
                let upto = remaining * weighted / weights;
                let share = upto - given;
                given = upto;
                // A share below the child's own floor is not a share it can
                // take; the deficit pass below settles the shortfall.
                share.max(claim.floor)
            }
            _ => claim.demand,
        });
    }

    let mut excess = assigned.iter().sum::<usize>().saturating_sub(area);
    for group in [Kind::Fill(0), Kind::Auto, Kind::Cells] {
        if excess == 0 {
            break;
        }
        excess = shrink(&mut assigned, claims, group, excess);
    }
    assigned
}

impl Claim {
    const fn weight(&self) -> usize {
        match self.kind {
            Kind::Fill(weight) => weight as usize,
            _ => 0,
        }
    }
}

/// Takes `excess` cells from one group, proportionally, and returns what it
/// could not take because every member sat on its floor.
fn shrink(assigned: &mut [usize], claims: &[Claim], group: Kind, mut excess: usize) -> usize {
    let holds = |kind: Kind| match group {
        Kind::Fill(_) => matches!(kind, Kind::Fill(_)),
        other => kind == other,
    };
    while excess > 0 {
        let active: Vec<usize> = (0..claims.len())
            .filter(|&index| holds(claims[index].kind) && assigned[index] > claims[index].floor)
            .collect();
        let total: usize = active.iter().map(|&index| assigned[index]).sum();
        if total == 0 {
            break;
        }
        let mut removed = 0;
        for &index in &active {
            if removed == excess {
                break;
            }
            // Rounding up keeps every pass making progress, and the remaining
            // excess caps what the last children give.
            let cut = (excess * assigned[index])
                .div_ceil(total)
                .min(excess - removed)
                .min(assigned[index] - claims[index].floor);
            assigned[index] -= cut;
            removed += cut;
        }
        if removed == 0 {
            break;
        }
        excess -= removed;
    }
    excess
}

/// The kind of claim a view makes on a `Row`'s main axis.
pub(super) fn width_kind(view: &View) -> Kind {
    match view {
        View::Block(style, _) => kind_of(style.width_length()),
        _ => Kind::Auto,
    }
}

/// The kind of claim a view makes on a `Column`'s main axis.
pub(super) fn height_kind(view: &View) -> Kind {
    match view {
        View::Block(style, _) => kind_of(style.height_length()),
        _ => Kind::Auto,
    }
}

const fn kind_of(length: Option<Length>) -> Kind {
    match length {
        Some(Length::Fill(weight)) => Kind::Fill(weight),
        Some(Length::Cells(_)) => Kind::Cells,
        None => Kind::Auto,
    }
}

/// The claims a `Row`'s children make on its width.
pub(super) fn width_claims(children: &[View]) -> Vec<Claim> {
    children
        .iter()
        .map(|child| Claim {
            kind: width_kind(child),
            demand: max_content_width(child),
            floor: min_content_width(child),
        })
        .collect()
}

/// Whether a view spans whatever area it is given, rather than taking an
/// intrinsic size.
///
/// A `Fill` length needs an area to divide, so a box that contains one spans
/// its own available extent. The height axis gets this for free — a box
/// resolves its height after its content, so a stretched child is already in
/// the row count — but the width is decided before the content is laid out,
/// which is why the width axis has to ask.
pub(super) fn fills_width(view: &View) -> bool {
    match view {
        View::Text(..) => false,
        View::Block(style, child) => match style.width_length() {
            Some(Length::Fill(_)) => true,
            Some(Length::Cells(_)) => false,
            None => fills_width(child),
        },
        View::Row(_, children) | View::Column(_, children) => children.iter().any(fills_width),
    }
}

/// Which parts of the frame an area forces away, on one axis.
///
/// Margin collapses first, then padding, and only by what the area cannot
/// hold. Below that the content gives way, and only an area that cannot hold
/// the frame itself reaches the safety net.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Degraded {
    pub margin: bool,
    pub padding: bool,
}

/// Decides that degradation for one axis.
pub(super) fn degrade(
    area: Option<usize>,
    margin: usize,
    border: usize,
    padding: usize,
    min_content: usize,
) -> Degraded {
    let Some(area) = area else {
        return Degraded {
            margin: false,
            padding: false,
        };
    };
    if margin + border + padding + min_content <= area {
        return Degraded {
            margin: false,
            padding: false,
        };
    }
    Degraded {
        margin: true,
        padding: border + padding + min_content > area,
    }
}

/// The cells the enabled border edges contribute, per axis.
pub(super) fn border_extent(style: &BlockStyle) -> Size {
    match style.border_kind() {
        Some(_) => Size::new(
            usize::from(style.is_border_left_enabled())
                + usize::from(style.is_border_right_enabled()),
            usize::from(style.is_border_top_enabled())
                + usize::from(style.is_border_bottom_enabled()),
        ),
        None => Size::ZERO,
    }
}

pub(super) const fn horizontal(sides: Sides) -> usize {
    sides.left as usize + sides.right as usize
}

pub(super) const fn vertical(sides: Sides) -> usize {
    sides.top as usize + sides.bottom as usize
}

/// The width a view takes when nothing bounds it: its max-content size.
///
/// This measures without assembling a rectangle, so a block resolves its own
/// width in one downward pass instead of laying its child out twice.
pub(super) fn max_content_width(view: &View) -> usize {
    match view {
        View::Text(text, _) => text_lines(text)
            .iter()
            .map(|line| line.width())
            .max()
            .unwrap_or(0),
        View::Block(style, child) => {
            let axis = width_axis(style, style.frame_size().width());
            let used = axis.used(None, max_content_width(child), min_content_width(child));
            used + horizontal(style.margin_sides())
        }
        View::Row(_, children) => children.iter().map(max_content_width).sum(),
        View::Column(_, children) => children.iter().map(max_content_width).max().unwrap_or(0),
    }
}

/// The width below which a view cannot go without splitting a grapheme.
pub(super) fn min_content_width(view: &View) -> usize {
    match view {
        View::Text(text, _) => text_lines(text)
            .iter()
            .flat_map(|line| {
                line.graphemes()
                    .map(PrintableText::width)
                    .collect::<Vec<_>>()
            })
            .max()
            .unwrap_or(0),
        View::Block(style, child) => {
            let frame = style.frame_size().width();
            let floor = style
                .minimum_width()
                .map_or(0, usize::from)
                .max(frame + min_content_width(child));
            floor + horizontal(style.margin_sides())
        }
        View::Row(_, children) => children.iter().map(min_content_width).sum(),
        View::Column(_, children) => children.iter().map(min_content_width).max().unwrap_or(0),
    }
}

/// The height below which a view cannot be shrunk.
///
/// The axes are not symmetric: a row can simply be absent, so a box's height
/// floor is its frame — plus whatever frame its own content cannot give up —
/// rather than a content extent.
pub(super) fn min_content_height(view: &View) -> usize {
    match view {
        View::Text(..) => 0,
        View::Block(style, child) => {
            let frame = style.frame_size().height();
            let floor = style
                .minimum_height()
                .map_or(0, usize::from)
                .max(frame + min_content_height(child));
            floor + vertical(style.margin_sides())
        }
        View::Row(_, children) => children.iter().map(min_content_height).max().unwrap_or(0),
        View::Column(_, children) => children.iter().map(min_content_height).sum(),
    }
}

/// The width axis a block's style states, for a frame of `frame` cells.
pub(super) fn width_axis(style: &BlockStyle, frame: usize) -> Axis {
    Axis {
        frame,
        length: style.width_length(),
        min: style.minimum_width(),
        max: style.maximum_width(),
    }
}

/// The height axis a block's style states, for a frame of `frame` cells.
pub(super) fn height_axis(style: &BlockStyle, frame: usize) -> Axis {
    Axis {
        frame,
        length: style.height_length(),
        min: style.minimum_height(),
        max: style.maximum_height(),
    }
}

/// Splits a view's text into lines, keeping one empty line for empty text so a
/// box always has a content row to place.
///
/// The argument is plain text: a [`View::Text`] node never holds escape
/// sequences, which is what lets the layout pass measure without scanning.
pub(super) fn text_lines(text: &str) -> Vec<&PrintableText> {
    PrintableLines::new(text).lines()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Border, TextStyle};

    fn text(content: &str) -> View {
        View::text(content, TextStyle::new())
    }

    #[test]
    fn the_floor_wins_over_the_cap() {
        assert_eq!(clamp_size(10, Some(4), 0), 4, "the cap binds");
        assert_eq!(clamp_size(2, Some(4), 0), 2, "neither binds");
        assert_eq!(clamp_size(2, None, 6), 6, "the floor binds");
        assert_eq!(clamp_size(10, Some(4), 6), 6, "the floor beats the cap");
    }

    #[test]
    fn the_tighter_bound_wins_and_an_absent_one_never_binds() {
        assert_eq!(tighter(Some(4), Some(9)), Some(4));
        assert_eq!(tighter(Some(9), Some(4)), Some(4));
        assert_eq!(tighter(Some(4), None), Some(4));
        assert_eq!(tighter(None, Some(4)), Some(4));
        assert_eq!(tighter(None, None), None);
    }

    #[test]
    fn an_axis_sizes_from_the_content_when_no_length_is_stated() {
        let axis = Axis {
            frame: 2,
            length: None,
            min: None,
            max: None,
        };

        assert_eq!(axis.used(None, 5, 1), 7, "the frame is inside the size");
        assert_eq!(axis.used(Some(4), 5, 1), 4, "the area caps it");
        assert_eq!(axis.used(Some(9), 5, 1), 7, "a wider area does not pad it");
    }

    #[test]
    fn a_stated_length_is_the_base_the_bounds_clamp() {
        let axis = |length, min, max| Axis {
            frame: 0,
            length,
            min,
            max,
        };

        assert_eq!(
            axis(Some(Length::Cells(6)), None, None).used(None, 2, 0),
            6,
            "a stated size ignores the content"
        );
        assert_eq!(
            axis(Some(Length::Cells(6)), None, Some(4)).used(None, 2, 0),
            4,
            "the maximum caps the stated size"
        );
        assert_eq!(
            axis(Some(Length::Cells(6)), Some(8), None).used(None, 2, 0),
            8,
            "the minimum floors it"
        );
        assert_eq!(
            axis(None, None, Some(4)).used(Some(9), 2, 0),
            2,
            "a maximum alone is shrink-to-fit with a cap"
        );
    }

    #[test]
    fn a_fill_length_takes_the_area_and_falls_back_to_the_content() {
        let axis = Axis {
            frame: 2,
            length: Some(Length::Fill(1)),
            min: None,
            max: None,
        };

        assert_eq!(axis.used(Some(10), 3, 1), 10, "it spans the area");
        assert_eq!(
            axis.used(None, 3, 1),
            5,
            "with no area it contributes the intrinsic size"
        );
    }

    #[test]
    fn the_content_floor_is_the_axis_asymmetry() {
        let axis = Axis {
            frame: 2,
            length: Some(Length::Cells(1)),
            min: None,
            max: None,
        };

        // Width passes the widest unsplittable grapheme, so the box cannot
        // reach the stated size.
        assert_eq!(axis.used(Some(1), 4, 2), 4);
        // Height passes zero, so it can: the frame alone is a valid box.
        assert_eq!(axis.used(Some(1), 4, 0), 2);
    }

    #[test]
    fn the_content_bound_is_the_tightest_of_the_size_the_maximum_and_the_area() {
        let axis = |length, max| Axis {
            frame: 2,
            length,
            min: None,
            max,
        };

        assert_eq!(axis(None, None).content_bound(None), None, "nothing bounds");
        assert_eq!(axis(None, None).content_bound(Some(9)), Some(7));
        assert_eq!(
            axis(Some(Length::Cells(5)), None).content_bound(None),
            Some(3)
        );
        assert_eq!(
            axis(Some(Length::Cells(5)), Some(4)).content_bound(Some(9)),
            Some(2),
            "the maximum is tighter than the stated size"
        );
        assert_eq!(
            axis(Some(Length::Fill(1)), None).content_bound(Some(6)),
            Some(4),
            "a Fill length is bounded by the area it fills"
        );
        assert_eq!(
            axis(Some(Length::Cells(1)), None).content_bound(None),
            Some(0),
            "a size below the frame leaves no content, not a negative one"
        );
    }

    #[test]
    fn degradation_collapses_margin_before_padding_and_only_under_pressure() {
        // 2 margin + 2 border + 2 padding + 1 content = 7.
        let at = |area| degrade(area, 2, 2, 2, 1);

        assert_eq!(
            at(None),
            Degraded {
                margin: false,
                padding: false
            },
            "an unbounded axis never degrades"
        );
        assert_eq!(
            at(Some(7)),
            Degraded {
                margin: false,
                padding: false
            },
            "exactly enough is enough"
        );
        assert_eq!(
            at(Some(6)),
            Degraded {
                margin: true,
                padding: false
            },
            "the margin goes first"
        );
        assert_eq!(
            at(Some(5)),
            Degraded {
                margin: true,
                padding: false
            },
            "the box itself still fits"
        );
        assert_eq!(
            at(Some(4)),
            Degraded {
                margin: true,
                padding: true
            },
            "then the padding"
        );
        assert_eq!(
            at(Some(1)),
            Degraded {
                margin: true,
                padding: true
            },
            "below that the content and the safety net take over"
        );
    }

    fn claim(kind: Kind, demand: usize, floor: usize) -> Claim {
        Claim {
            kind,
            demand,
            floor,
        }
    }

    #[test]
    fn fill_children_divide_what_the_others_leave() {
        let stated = claim(Kind::Cells, 6, 0);
        let auto = claim(Kind::Auto, 4, 0);
        let fill = |weight| claim(Kind::Fill(weight), 0, 0);

        assert_eq!(distribute(20, &[stated, fill(1)]), vec![6, 14]);
        assert_eq!(distribute(20, &[stated, auto, fill(1)]), vec![6, 4, 10]);
        assert_eq!(distribute(10, &[fill(1), fill(1)]), vec![5, 5]);
        assert_eq!(distribute(9, &[fill(1), fill(2)]), vec![3, 6]);
    }

    #[test]
    fn a_share_never_loses_a_cell_to_rounding() {
        let fill = claim(Kind::Fill(1), 0, 0);

        // Three equal weights over ten cells: the odd cell goes to the last,
        // and the shares still sum to the area.
        assert_eq!(distribute(10, &[fill, fill, fill]), vec![3, 3, 4]);
        for area in 0..40 {
            let shares = distribute(area, &[fill, fill, fill]);
            assert_eq!(shares.iter().sum::<usize>(), area, "at {area}");
        }
    }

    #[test]
    fn nothing_is_redistributed_when_a_child_leaves_slack() {
        // A capped Fill child is capped by its own clamp, not by distribution:
        // the share is handed over whole and the remainder simply goes unused.
        assert_eq!(
            distribute(20, &[claim(Kind::Fill(1), 0, 0), claim(Kind::Cells, 4, 0)]),
            vec![16, 4]
        );
    }

    #[test]
    fn a_deficit_shrinks_fill_then_auto_then_stated() {
        let claims = [
            claim(Kind::Cells, 6, 3),
            claim(Kind::Auto, 6, 3),
            claim(Kind::Fill(1), 0, 3),
        ];

        assert_eq!(distribute(16, &claims), vec![6, 6, 4], "no deficit at all");
        // The Fill child is already at its floor, so the automatic child gives
        // way next, and the stated one only once that floor binds too.
        assert_eq!(distribute(14, &claims), vec![6, 5, 3]);
        assert_eq!(distribute(12, &claims), vec![6, 3, 3]);
        assert_eq!(distribute(10, &claims), vec![4, 3, 3]);
        // Below the sum of the floors every child is frozen and the container
        // resolves larger than its area, where the safety net takes over.
        assert_eq!(distribute(4, &claims), vec![3, 3, 3]);
    }

    #[test]
    fn a_deficit_within_one_group_is_proportional_and_freezes_at_a_floor() {
        let claims = [claim(Kind::Auto, 12, 0), claim(Kind::Auto, 4, 3)];

        // Twice the size gives twice the cells, until the smaller child hits
        // its floor and the rest of the shortfall falls on the larger one.
        assert_eq!(distribute(12, &claims), vec![9, 3]);
        assert_eq!(distribute(8, &claims), vec![5, 3]);
    }

    #[test]
    fn a_view_fills_its_area_when_anything_inside_it_does() {
        let filling = View::block(BlockStyle::new().width(Length::Fill(1)), text("a"));

        assert!(!fills_width(&text("a")));
        assert!(fills_width(&filling));
        assert!(
            fills_width(&View::block(BlockStyle::new(), filling.clone())),
            "an automatic box inherits its content's appetite"
        );
        assert!(
            !fills_width(&View::block(BlockStyle::new().width(4), filling.clone())),
            "a stated size settles the box, whatever it contains"
        );
        assert!(fills_width(&View::row(
            crate::VerticalAlign::Top,
            [text("a"), filling]
        )));
    }

    #[test]
    fn a_height_floor_is_the_frame_and_a_width_floor_is_the_content() {
        let bordered = View::block(BlockStyle::new().border(Border::NORMAL), text("abc"));

        assert_eq!(min_content_height(&text("abc")), 0, "a row can be absent");
        assert_eq!(min_content_height(&bordered), 2, "the frame cannot");
        assert_eq!(min_content_width(&bordered), 3, "one grapheme plus it");

        let floored = View::block(BlockStyle::new().min_height(5), text("a"));
        assert_eq!(min_content_height(&floored), 5);

        let spaced = View::block(BlockStyle::new().margin((1, 0)), text("a"));
        assert_eq!(min_content_height(&spaced), 2, "margin lies outside");
    }

    #[test]
    fn border_extent_counts_only_enabled_edges_of_a_present_border() {
        assert_eq!(border_extent(&BlockStyle::new()), Size::ZERO);

        let full = BlockStyle::new().border(Border::NORMAL);
        assert_eq!(border_extent(&full), Size::new(2, 2));

        let sides = full.clone().border_top(false).border_left(false);
        assert_eq!(border_extent(&sides), Size::new(1, 1));

        let none = BlockStyle::new()
            .border(Border::NORMAL)
            .border_top(false)
            .border_right(false)
            .border_bottom(false)
            .border_left(false);
        assert_eq!(border_extent(&none), Size::ZERO);
    }

    #[test]
    fn text_is_measured_by_its_widest_line_and_its_widest_grapheme() {
        assert_eq!(max_content_width(&text("ab\nabcd\nabc")), 4);
        assert_eq!(min_content_width(&text("ab\nabcd\nabc")), 1);

        assert_eq!(max_content_width(&text("日本語")), 6);
        assert_eq!(
            min_content_width(&text("日本語")),
            2,
            "a wide character cannot be split"
        );
        assert_eq!(min_content_width(&text("a👩‍💻")), 2, "nor can a cluster");

        assert_eq!(max_content_width(&text("")), 0);
        assert_eq!(min_content_width(&text("")), 0);
    }

    #[test]
    fn a_block_measures_its_frame_bounds_and_margin() {
        let bordered = View::block(
            BlockStyle::new().border(Border::NORMAL).padding((0, 1)),
            text("abc"),
        );
        assert_eq!(
            max_content_width(&bordered),
            7,
            "3 content + 2 padding + 2 border"
        );
        assert_eq!(
            min_content_width(&bordered),
            5,
            "one grapheme plus the frame"
        );

        let bounded = View::block(BlockStyle::new().max_width(2), text("abcd"));
        assert_eq!(
            max_content_width(&bounded),
            2,
            "the maximum caps the measure"
        );

        let floored = View::block(BlockStyle::new().min_width(9), text("abcd"));
        assert_eq!(max_content_width(&floored), 9);
        assert_eq!(min_content_width(&floored), 9, "the minimum is a floor too");

        let spaced = View::block(BlockStyle::new().margin((0, 2)), text("ab"));
        assert_eq!(max_content_width(&spaced), 6, "margin lies outside the box");
        assert_eq!(min_content_width(&spaced), 5);
    }

    #[test]
    fn a_row_sums_its_children_and_a_column_takes_the_widest() {
        let children = [text("abcd"), text("日本")];
        let row = View::row(crate::VerticalAlign::Top, children.clone());
        let column = View::column(crate::Align::Left, children);

        assert_eq!(max_content_width(&row), 8);
        assert_eq!(min_content_width(&row), 3, "one grapheme from each child");
        assert_eq!(max_content_width(&column), 4);
        assert_eq!(min_content_width(&column), 2);
    }
}
