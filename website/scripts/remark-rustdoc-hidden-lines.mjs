/**
 * Apply rustdoc's hidden-line convention to Rust code fences rendered by Astro.
 *
 * Rustdoc compiles lines prefixed with `# ` but omits them from the displayed
 * snippet. Markdown renderers do not know that convention, so without this
 * transform the marker is shown and copied as invalid Rust source.
 */
export default {
  name: 'rustdoc-hidden-lines',
  code(node, context) {
    if (node.lang !== 'rust') return;
    context.setProperty(
      node,
      'value',
      node.value
      .split('\n')
      .filter((line) => !/^#(?: |$)/.test(line))
      .join('\n'),
    );
  },
};
