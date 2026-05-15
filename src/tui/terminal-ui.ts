import * as p from '@clack/prompts';
import { APP_NAME } from '../app-identity';
import { colorize, oneDark, semantic } from './colors';

export function printCommandHeader(command: string, subtitle?: string): void {
  p.intro(colorize(`${APP_NAME} ${command}`, oneDark.blue));
  if (subtitle) {
    p.log.info(colorize(subtitle, semantic.subtitle));
  }
}

export function formatKeyValue(key: string, value: string): string {
  return `${colorize(`${key}:`, semantic.subtitle)} ${colorize(value, oneDark.text)}`;
}

export function printKeyValues(title: string, rows: Array<[string, string]>): void {
  p.note(rows.map(([key, value]) => formatKeyValue(key, value)).join('\n'), colorize(title, semantic.title));
}

export function printWarning(title: string, lines: string[]): void {
  p.note(lines.map((line) => colorize(line, semantic.warning)).join('\n'), colorize(title, semantic.warning));
}
