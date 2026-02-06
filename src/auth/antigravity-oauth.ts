/**
 * Antigravity OAuth flow (Google Cloud)
 * Uses the same public credentials as OpenCode/pi-ai
 */
import { randomBytes, createHash } from 'crypto';

// Public OAuth credentials (same as pi-ai/OpenCode)
const CLIENT_ID = '1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com';
const CLIENT_SECRET = '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';
const REDIRECT_URI = 'http://localhost:51121/oauth-callback';

const SCOPES = [
  'https://www.googleapis.com/auth/cloud-platform',
  'https://www.googleapis.com/auth/userinfo.email',
  'https://www.googleapis.com/auth/userinfo.profile',
];

const AUTH_URL = 'https://accounts.google.com/o/oauth2/v2/auth';
const TOKEN_URL = 'https://oauth2.googleapis.com/token';
const USERINFO_URL = 'https://www.googleapis.com/oauth2/v2/userinfo';

export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  token_type: string;
}

export interface UserInfo {
  email: string;
  name?: string;
}

export interface OAuthResult {
  accessToken: string;
  refreshToken: string;
  expiresAt: number;
  email: string;
  name?: string;
}

function generatePKCE(): { verifier: string; challenge: string } {
  const verifier = randomBytes(32).toString('base64url');
  const challenge = createHash('sha256').update(verifier).digest('base64url');
  return { verifier, challenge };
}

function generateState(): string {
  return randomBytes(16).toString('hex');
}

/**
 * Start local callback server to receive OAuth code
 */
async function startCallbackServer(): Promise<{
  waitForCode: () => Promise<{ code: string; state: string } | null>;
  close: () => void;
}> {
  const server = Bun.serve({
    port: 51121,
    hostname: '127.0.0.1',
    fetch(req) {
      const url = new URL(req.url);
      
      if (url.pathname === '/oauth-callback') {
        const code = url.searchParams.get('code');
        const state = url.searchParams.get('state');
        const error = url.searchParams.get('error');

        if (error) {
          return new Response(
            `<html><body><h1>Authentication Failed</h1><p>Error: ${error}</p><p>You can close this window.</p></body></html>`,
            { headers: { 'Content-Type': 'text/html' }, status: 400 }
          );
        }

        if (code && state) {
          // Store result for polling
          (server as any).__result = { code, state };
          return new Response(
            `<html><body><h1>Authentication Successful!</h1><p>You can close this window and return to the terminal.</p></body></html>`,
            { headers: { 'Content-Type': 'text/html' } }
          );
        }

        return new Response('Missing parameters', { status: 400 });
      }

      return new Response('Not found', { status: 404 });
    },
  });

  return {
    waitForCode: async () => {
      // Poll for result with timeout (2 minutes)
      const timeout = Date.now() + 120_000;
      while (Date.now() < timeout) {
        const result = (server as any).__result;
        if (result) {
          return result;
        }
        await Bun.sleep(100);
      }
      return null;
    },
    close: () => server.stop(),
  };
}

/**
 * Exchange authorization code for tokens
 */
async function exchangeCodeForTokens(code: string, codeVerifier: string): Promise<TokenResponse> {
  const response = await fetch(TOKEN_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      client_id: CLIENT_ID,
      client_secret: CLIENT_SECRET,
      code,
      code_verifier: codeVerifier,
      grant_type: 'authorization_code',
      redirect_uri: REDIRECT_URI,
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Token exchange failed: ${error}`);
  }

  return response.json();
}

/**
 * Get user info from access token
 */
async function getUserInfo(accessToken: string): Promise<UserInfo> {
  const response = await fetch(USERINFO_URL, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });

  if (!response.ok) {
    throw new Error('Failed to get user info');
  }

  return response.json();
}

/**
 * Refresh access token using refresh token
 */
export async function refreshAccessToken(refreshToken: string): Promise<{ accessToken: string; expiresAt: number }> {
  const response = await fetch(TOKEN_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      client_id: CLIENT_ID,
      client_secret: CLIENT_SECRET,
      refresh_token: refreshToken,
      grant_type: 'refresh_token',
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Token refresh failed: ${error}`);
  }

  const data: TokenResponse = await response.json();
  return {
    accessToken: data.access_token,
    expiresAt: Date.now() + data.expires_in * 1000,
  };
}

/**
 * Run the full OAuth flow
 * Returns tokens and user info
 */
export async function runAntigravityOAuth(openBrowser: (url: string) => Promise<void>): Promise<OAuthResult> {
  // Generate PKCE and state
  const { verifier, challenge } = generatePKCE();
  const state = generateState();

  // Start callback server
  const server = await startCallbackServer();

  try {
    // Build auth URL
    const authUrl = new URL(AUTH_URL);
    authUrl.searchParams.set('client_id', CLIENT_ID);
    authUrl.searchParams.set('redirect_uri', REDIRECT_URI);
    authUrl.searchParams.set('response_type', 'code');
    authUrl.searchParams.set('scope', SCOPES.join(' '));
    authUrl.searchParams.set('state', state);
    authUrl.searchParams.set('code_challenge', challenge);
    authUrl.searchParams.set('code_challenge_method', 'S256');
    authUrl.searchParams.set('access_type', 'offline');
    authUrl.searchParams.set('prompt', 'consent');

    // Open browser
    await openBrowser(authUrl.toString());

    // Wait for callback
    const result = await server.waitForCode();
    
    if (!result) {
      throw new Error('OAuth timed out - no response received');
    }

    if (result.state !== state) {
      throw new Error('OAuth state mismatch - possible CSRF attack');
    }

    // Exchange code for tokens
    const tokens = await exchangeCodeForTokens(result.code, verifier);

    // Get user info
    const userInfo = await getUserInfo(tokens.access_token);

    return {
      accessToken: tokens.access_token,
      refreshToken: tokens.refresh_token,
      expiresAt: Date.now() + tokens.expires_in * 1000,
      email: userInfo.email,
      name: userInfo.name,
    };
  } finally {
    server.close();
  }
}
