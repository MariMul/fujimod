import type { RequestEvent } from '@solidjs/web';
import { runtimeHeaders } from './security';
export default async function middleware(_event: RequestEvent, next: () => Promise<Response>) {
  const response = await next();
  for (const [name, value] of Object.entries(runtimeHeaders)) response.headers.set(name, value);
  return response;
}
