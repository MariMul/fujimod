import { defineConfig } from 'vite-plus';
import { uiLint } from './lint';

export default defineConfig({ lint: uiLint, fmt: { singleQuote: true } });
