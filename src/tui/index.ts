import * as p from '@clack/prompts';
import { catppuccin, semantic, colorize, bold } from './colors';
import { showListAll } from './list-all';
import { configureWaybar } from './configure-waybar';
import { configureTooltip } from './configure-tooltip';
import { loginProviderFlow } from './login';

const VERSION = '3.0.0';

type MenuAction = 'list' | 'waybar' | 'tooltip' | 'login' | 'cancel';

async function showMainMenu(): Promise<MenuAction | symbol> {
  console.log('');
  console.log(colorize('  Tip: Use arrow keys to navigate, Enter to select, q to quit', semantic.subtitle));
  console.log('');

  const result = await p.select({
    message: 'What would you like to do?',
    options: [
      { 
        value: 'list' as const, 
        label: 'List all',
        hint: 'view quotas for all logged providers',
      },
      { 
        value: 'waybar' as const, 
        label: 'Configure Waybar',
        hint: 'select providers for the bar',
      },
      { 
        value: 'tooltip' as const, 
        label: 'Configure Tooltip',
        hint: 'select what shows on hover',
      },
      {
        value: 'login' as const,
        label: 'Provider login',
        hint: 'launch provider CLI login flows',
      },
      { 
        value: 'cancel' as const, 
        label: 'Cancel',
      },
    ],
  });

  return result as MenuAction | symbol;
}

export async function runTui(): Promise<void> {
  console.clear();
  
  p.intro(colorize(`qbar v${VERSION}`, semantic.accent));

  let running = true;

  while (running) {
    const action = await showMainMenu();

    if (p.isCancel(action) || action === 'cancel') {
      running = false;
      continue;
    }

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
