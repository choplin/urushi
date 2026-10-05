// @ts-check
import { defineConfig } from 'astro/config';
import { satteri } from '@astrojs/markdown-satteri';
import starlight from '@astrojs/starlight';
import { cloudflareBuildOutput } from './scripts/cloudflare-build-output.mjs';
import rustdocHiddenLines from './scripts/remark-rustdoc-hidden-lines.mjs';

export default defineConfig({
  site: 'https://urushi.choplin.dev',
  markdown: {
    processor: satteri({ mdastPlugins: [rustdocHiddenLines] }),
  },
  integrations: [
    starlight({
      title: 'Urushi',
      description:
        'A shared presentation model for Rust terminal applications.',
      favicon: '/brand/urushi-icon.png',
      disable404Route: true,
      customCss: ['./src/styles/starlight.css'],
      components: {
        Header: './src/components/DocsHeader.astro',
        SocialIcons: './src/components/DocsSocialLinks.astro',
      },
      sidebar: [
        {
          label: 'Overview',
          items: [
            { label: 'Introduction', slug: 'docs' },
            { label: 'Quickstart', slug: 'docs/quickstart' },
            { label: 'Choose by use case', slug: 'docs/use-cases' },
          ],
        },
        {
          label: 'Presentation model',
          items: [
            { label: 'Overview', slug: 'docs/concepts/presentation-foundation' },
            { label: 'View', slug: 'docs/core/views' },
            {
              label: 'Styles',
              collapsed: true,
              items: [
                { label: 'Overview', slug: 'docs/core/styles' },
                { label: 'Text appearance and links', slug: 'docs/core/styles/text' },
                { label: 'Reference', slug: 'docs/core/styles/reference' },
              ],
            },
            {
              label: 'Layout',
              collapsed: true,
              items: [
                { label: 'Overview', slug: 'docs/core/layout' },
                { label: 'Block geometry', slug: 'docs/core/styles/blocks' },
                { label: 'Rows and columns', slug: 'docs/core/views/blocks-and-layouts' },
                { label: 'Grid', slug: 'docs/core/views/grid' },
                { label: 'Viewports and anchors', slug: 'docs/core/views/projection-and-anchors' },
                { label: 'Reference', slug: 'docs/core/views/reference' },
              ],
            },
            {
              label: 'Themes',
              collapsed: true,
              items: [
                { label: 'Overview', slug: 'docs/core/themes' },
                { label: 'Light and dark themes', slug: 'docs/core/themes/adaptive' },
                { label: 'Custom themes and roles', slug: 'docs/core/themes/customize' },
                { label: 'Reference', slug: 'docs/core/themes/reference' },
              ],
            },
            {
              label: 'Components',
              collapsed: true,
              items: [
                { label: 'Overview', slug: 'docs/core/components' },
                { label: 'Lists', slug: 'docs/core/components/list' },
                { label: 'Tables', slug: 'docs/core/components/table' },
                { label: 'Trees', slug: 'docs/core/components/tree' },
                { label: 'Scrollbars', slug: 'docs/core/components/scrollbar' },
                { label: 'Custom presentations', slug: 'docs/core/components/custom-presentations' },
                { label: 'Reference', slug: 'docs/core/components/reference' },
              ],
            },
            {
              label: 'Canvas',
              collapsed: true,
              items: [
                { label: 'Overview', slug: 'docs/core/canvas' },
                { label: 'Sizing and placement', slug: 'docs/core/canvas/size-and-place' },
                { label: 'Text, Views, and cells', slug: 'docs/core/canvas/draw-content' },
                { label: 'Paths and line networks', slug: 'docs/core/canvas/draw-paths' },
                { label: 'Composition and clipping', slug: 'docs/core/canvas/composition' },
                { label: 'Custom CanvasItem', slug: 'docs/core/canvas/custom-items' },
                { label: 'Components on Canvas', slug: 'docs/core/canvas/components' },
                { label: 'Reference', slug: 'docs/core/canvas/reference' },
              ],
            },
            {
              label: 'Text width',
              collapsed: true,
              items: [
                { label: 'Overview', slug: 'docs/core/text-width' },
                { label: 'Tabs and wrapping', slug: 'docs/core/text-width/tabs-and-wrapping' },
                { label: 'Reference', slug: 'docs/core/text-width/reference' },
              ],
            },
          ],
        },
        {
          label: 'CLI output',
          items: [
            { label: 'Overview', slug: 'docs/cli' },
            { label: 'Styled output', slug: 'docs/cli/styled-output' },
            { label: 'Blocks and layouts', slug: 'docs/cli/layout' },
            { label: 'CLI presentations', slug: 'docs/cli/presentations' },
            { label: 'Output behavior', slug: 'docs/cli/output-behavior' },
            { label: 'Reference', slug: 'docs/cli/reference' },
          ],
        },
        {
          label: 'Prompts',
          items: [
            { label: 'Overview', slug: 'docs/prompts' },
            { label: 'Forms', slug: 'docs/prompts/form' },
            { label: 'Fields and validation', slug: 'docs/prompts/fields' },
            { label: 'Placement and resize', slug: 'docs/prompts/placement' },
            { label: 'Reference', slug: 'docs/prompts/reference' },
          ],
        },
        {
          label: 'TUI',
          items: [
            { label: 'Overview', slug: 'docs/tui' },
            { label: 'Build your first application', slug: 'docs/tui/application' },
            { label: 'Application runtime', slug: 'docs/tui/runtime' },
            { label: 'Effects and subscriptions', slug: 'docs/tui/effects-and-subscriptions' },
            { label: 'Message delivery and drawing', slug: 'docs/tui/delivery-and-drawing' },
            { label: 'Native Screen and Frame', slug: 'docs/tui/screen-and-frame' },
            { label: 'Ratatui adapter', slug: 'docs/tui/ratatui' },
            { label: 'Reference', slug: 'docs/tui/reference' },
          ],
        },
        {
          label: 'Graphics',
          items: [
            { label: 'Overview', slug: 'docs/graphics' },
            { label: 'Rendering and lifecycle', slug: 'docs/graphics/rendering-and-lifecycle' },
            { label: 'Reference', slug: 'docs/graphics/reference' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Crates and features', slug: 'docs/reference/crates' },
            { label: 'Rendering behavior', slug: 'docs/reference/rendering' },
            { label: 'Current limitations', slug: 'docs/reference/limitations' },
          ],
        },
      ],
    }),
    cloudflareBuildOutput(),
  ],
});
