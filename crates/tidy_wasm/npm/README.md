# tidyrs-wasm

[tidy](https://tidy.lukeed.com/), the code formatter with Prettier's style, as WebAssembly: for browsers, Workers and Deno, with the same calls and types as [`tidyrs`](https://www.npmjs.com/package/tidyrs).

```js
import init, { format } from "tidyrs-wasm";

await init();

format("a{color:red}", { lang: "css" });
//=> "a {\n  color: red;\n}\n"
```

Docs: https://tidy.lukeed.com/docs/wasm/
