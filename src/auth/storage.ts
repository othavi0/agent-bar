/**
 * Storage for qbar auth tokens
 * Stores in ~/.config/qbar/auth.json
 */
import { mkdir } from 'fs/promises';
import { join } from 'path';
import { homedir } from 'os';

const XDG_CONFIG_HOME = Bun.env.XDG_CONFIG_HOME || join(homedir(), '.config');
const AUTH_DIR = join(XDG_CONFIG_HOME, 'qbar');
const AUTH_FILE = join(AUTH_DIR, 'auth.json');

export interface AntigravityAccount {
  email: string;
  name?: string;
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
  addedAt: number;
}

export interface AuthStorage {
  version: number;
  antigravity: AntigravityAccount[];
}

const DEFAULT_STORAGE: AuthStorage = {
  version: 1,
  antigravity: [],
};

export async function loadAuthStorage(): Promise<AuthStorage> {
  const file = Bun.file(AUTH_FILE);
  
  if (!await file.exists()) {
    return { ...DEFAULT_STORAGE };
  }

  try {
    const data = await file.json();
    return {
      ...DEFAULT_STORAGE,
      ...data,
    };
  } catch {
    return { ...DEFAULT_STORAGE };
  }
}

export async function saveAuthStorage(storage: AuthStorage): Promise<void> {
  await mkdir(AUTH_DIR, { recursive: true });
  await Bun.write(AUTH_FILE, JSON.stringify(storage, null, 2));
}

export async function addAntigravityAccount(account: Omit<AntigravityAccount, 'addedAt'>): Promise<void> {
  const storage = await loadAuthStorage();
  
  // Remove existing account with same email
  storage.antigravity = storage.antigravity.filter(a => a.email !== account.email);
  
  // Add new account
  storage.antigravity.push({
    ...account,
    addedAt: Date.now(),
  });

  await saveAuthStorage(storage);
}

export async function getAntigravityAccount(): Promise<AntigravityAccount | null> {
  const storage = await loadAuthStorage();
  
  if (storage.antigravity.length === 0) {
    return null;
  }

  // Return most recently added
  return storage.antigravity.sort((a, b) => b.addedAt - a.addedAt)[0];
}

export async function removeAntigravityAccount(email: string): Promise<void> {
  const storage = await loadAuthStorage();
  storage.antigravity = storage.antigravity.filter(a => a.email !== email);
  await saveAuthStorage(storage);
}

export async function updateAntigravityToken(email: string, accessToken: string, expiresAt: number): Promise<void> {
  const storage = await loadAuthStorage();
  const account = storage.antigravity.find(a => a.email === email);
  
  if (account) {
    account.accessToken = accessToken;
    account.expiresAt = expiresAt;
    await saveAuthStorage(storage);
  }
}
