import type { RequestEvent } from '@solidjs/web';
import { contentSecurityPolicy } from './security';
export default async function middleware(_event: RequestEvent, next: () => Promise<Response>) {
  const response = await next();
  response.headers.set('Content-Security-Policy', contentSecurityPolicy);
  return response;
}
