import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { highlight } from '../code';

/**
 * The before and after that the home page shows. `routes.output.ts` is what
 * `tidy --no-config routes.input.ts` prints; `bun run check` fails if it drifts.
 */
let read = (name: string) => readFileSync(join(import.meta.dirname, '../samples', name), 'utf8').trimEnd();

export default {
	input: highlight(read('routes.input.ts'), 'typescript'),
	output: highlight(read('routes.output.ts'), 'typescript'),
	config: highlight(read('tidy.json'), 'json'),
};
