// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  integrations: [
    starlight({
      title: 'Urushi',
      description:
        'A shared presentation model for Rust terminal applications.',
      disable404Route: true,
      customCss: ['./src/styles/starlight.css'],
      components: {
        Header: './src/components/DocsHeader.astro',
      },
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'Introduction', slug: 'docs' },
            { label: 'Quickstart', slug: 'docs/quickstart' },
            { label: 'Choose by use case', slug: 'docs/use-cases' },
          ],
        },
        {
          label: 'CLI output',
          items: [
            { label: 'CLI overview', slug: 'docs/cli' },
            { label: 'Style ordinary output', slug: 'docs/cli/styled-output' },
            { label: 'Compose blocks and layouts', slug: 'docs/cli/layout' },
            { label: 'CLI presentations', slug: 'docs/cli/presentations' },
            { label: 'Output behavior', slug: 'docs/cli/output-behavior' },
          ],
        },
        {
          label: 'Prompts',
          items: [
            { label: 'Prompt overview', slug: 'docs/prompts' },
            { label: 'Build a form', slug: 'docs/prompts/form' },
            { label: 'Fields and validation', slug: 'docs/prompts/fields' },
            { label: 'Placement and resize', slug: 'docs/prompts/placement' },
          ],
        },
        {
          label: 'TUI',
          items: [
            { label: 'TUI overview', slug: 'docs/tui' },
            { label: 'Full-screen runtime', slug: 'docs/tui/runtime' },
            { label: 'Use the Ratatui adapter', slug: 'docs/tui/ratatui' },
          ],
        },
        {
          label: 'Graphics',
          items: [
            { label: 'Display terminal images', slug: 'docs/graphics' },
          ],
        },
        {
          label: 'Core model',
          items: [
            { label: 'Styles and views', slug: 'docs/core/styles-and-views' },
            { label: 'Themes', slug: 'docs/core/themes' },
            { label: 'Terminal width and CJK', slug: 'docs/core/text-width' },
            { label: 'Components', slug: 'docs/core/components' },
            { label: 'Presentation foundation', slug: 'docs/concepts/presentation-foundation' },
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
  ],
});
