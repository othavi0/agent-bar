import * as p from '@clack/prompts';
import { catppuccin, semantic, colorize } from './colors';
import { showListAll } from './list-all';
import { configureWaybar } from './configure-waybar';
import { configureTooltip } from './configure-tooltip';
import { loginProviderFlow } from './login';

const VERSION = '3.0.0';

type MenuAction = 'list' | 'waybar' | 'tooltip' | 'login';

export async function runTui(): Promise<void> {
  console.clear();
  
  // Timeline-style intro (OpenClaw-like)
  p.intro(colorize(`qbar ${colorize(`v${VERSION}`, semantic.subtitle)}`, semantic.accent));

  // Tips box (doesn't break the timeline)
  p.note(
    [
      colorize('↑↓', semantic.highlight) + ' navigate  ' +
      colorize('Enter', semantic.highlight) + ' select  ' +
      colorize('q', semantic.highlight) + ' quit',
    ].join('\n'),
    colorize('Controls', semantic.title)
  );

  let running = true;

  while (running) {
    const result = await p.select({
      message: colorize('What would you like to do?', semantic.title),
      options: [
        { 
          value: 'list' as const, 
          label: colorize('List all', catppuccin.text),
          hint: colorize('view quotas for all providers', semantic.muted),
        },
        { 
          value: 'waybar' as const, 
          label: colorize('Configure Waybar', catppuccin.text),
          hint: colorize('select providers for the bar', semantic.muted),
        },
        { 
          value: 'tooltip' as const, 
          label: colorize('Configure Tooltip', catppuccin.text),
          hint: colorize('select what shows on hover', semantic.muted),
        },
        {
          value: 'login' as const,
          label: colorize('Provider login', catppuccin.text),
          hint: colorize('launch provider CLI login flows', semantic.muted),
        },
      ],
    });

    // q or Ctrl+C exits
    if (p.isCancel(result)) {
      running = false;
      continue;
    }

    const action = result as MenuAction;

    // Log the step (timeline-style)
    p.log.step(colorize(`→ ${action}`, semantic.accent));

    switch (action) {
      case 'list':
        await showListAll();
        break;

      case 'waybar':
        await configureWaybar();
        break;

      case 'tooltip':
        await configureTooltip();
        break;

      case 'login':
        await loginProviderFlow();
        break;
    }
  }

  p.outro(colorize('Goodbye!', semantic.muted));
}

// Handle keyboard interrupt gracefully
process.on('SIGINT', () => {
  console.log('');
  p.outro(colorize('Cancelled', semantic.muted));
  process.exit(0);
});
