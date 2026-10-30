---
title: mixed
---

Setext Title
============

Intro with *emphasis*, __strong__, ~strike~, `code`, a [link](/a "t"), an ![image](/i.png),
an autolink <https://x.dev>, a footnote[^1], \*escaped\* text & entities &copy;, and <span>html</span>.

* bullet one
* bullet two
    * nested with four spaces
    * and a [reference][ref]

1) first
2) second

    indented code inside the second

> quote line
continued lazily
> - list in a quote
>   1. ordered in a list in a quote

| left | center | right |
|:-----|:------:|------:|
| a | b | c |
| longer cell | `x|y` | \| |

- [ ] todo
- [X] done

<div>
  html block

</div>

***

Term with trailing spaces  
and a hard break, then a backslash one\
here.

```js   meta
let x = "```";
```

~~~
tilde fenced
~~~

[^1]: The footnote,
    on two lines.

[ref]: https://example.com  "Title"
