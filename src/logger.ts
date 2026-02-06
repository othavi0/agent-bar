type LogLevel = 'debug' | 'info' | 'warn' | 'error';

const LOG_LEVELS: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

class Logger {
  private level: LogLevel = 'warn';
  private silent: boolean = false;

  setLevel(level: LogLevel) {
    this.level = level;
  }

  setSilent(silent: boolean) {
    this.silent = silent;
  }

  private shouldLog(level: LogLevel): boolean {
    if (this.silent && level !== 'error') return false;
    return LOG_LEVELS[level] >= LOG_LEVELS[this.level];
  }

  private formatMessage(level: LogLevel, message: string, meta?: object): string {
    const timestamp = new Date().toISOString();
    const prefix = `[${timestamp}] [${level.toUpperCase()}]`;
    if (meta) {
      return `${prefix} ${message} ${JSON.stringify(meta)}`;
    }
    return `${prefix} ${message}`;
  }

  debug(message: string, meta?: object) {
    if (this.shouldLog('debug')) {
      console.error(this.formatMessage('debug', message, meta));
    }
  }

  info(message: string, meta?: object) {
    if (this.shouldLog('info')) {
      console.error(this.formatMessage('info', message, meta));
    }
  }

  warn(message: string, meta?: object) {
    if (this.shouldLog('warn')) {
      console.error(this.formatMessage('warn', message, meta));
    }
  }

  error(message: string, meta?: object) {
    if (this.shouldLog('error')) {
      console.error(this.formatMessage('error', message, meta));
    }
  }
}

export const logger = new Logger();
