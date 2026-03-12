import ora, { type Options } from 'ora';

export function createSpinner(text: string, opts?: Partial<Options>) {
  return ora({
    text,
    spinner: 'dots',
    color: 'cyan',
    indent: 2,
    discardStdin: false,
    ...opts,
  });
}
