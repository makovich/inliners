# inline*rs* [![crates.io](https://img.shields.io/crates/v/inliners.svg)](https://crates.io/crates/inliners) [![Build status](https://travis-ci.org/makovich/inliners.svg?branch=master)](https://travis-ci.org/makovich/inliners)

Inline images, CSS, JavaScript and more into a single HTML web page. Quite fast. It is mostly influenced by Remi's [inliner](https://github.com/remy/inliner/).

> **WARNING: Works on my machine notice!**
> This project is a result of playing with [rayon](https://docs.rs/rayon/) and [kuchiki](https://docs.rs/kuchiki/) so still not feature complete &mdash; but can be easily extended.

## Features

* Inline local stuff (i.e. for development or packaging) and remote (i.e. archiving)
* Can load and inline assets in a very parallel fashion (use `-j THREADS` switch)
* Handles scripts and styles (`@import`, `<style />` or `<tag style="..."`)
* Encode images and favicons with Base64 (`<img src="i.jpg"/>`, `div { background-image: url('img/i.png'); }`)
* Minify output with [minify-html](https://github.com/wilsonzlin/minify-html) (can do scripts and styles with [esbuild](https://github.com/evanw/esbuild) **but might not work well and as expected**).

## Install

Use [releases page](https://github.com/makovich/inliners/releases) or install from crates.io with `cargo`:
```sh
# By default minifies only HTML:
$ cargo install inliners

# If you have Golang installed, allow JS/CSS minification:
$ cargo install inliners --features="esbuild"

# Or disable minification at all:
$ cargo install --no-default-features

# Then:
$ cd mysite
$ inline --no-img --minify index.html > index.min.html

# Or:
$ inline --no-js -o ~/archive/wiki/minipig.html https://en.wikipedia.org/wiki/Miniature_pig
```

## Usage

```
inline 0.5.0
Inline images, CSS, JavaScript and more into a single HTML web page. Quite fast.

USAGE:
    inline [FLAGS] [OPTIONS] [input]

FLAGS:
    -h, --help       Prints help information
    -m, --minify     Minify HTML, CSS and JavaScript
    -C, --no-css     Do not process/embedd CSS stylesheets
    -I, --no-img     Do not process/embedd images
    -J, --no-js      Do not process/embedd JavaScript
    -q, --quiet      Silence all output
    -V, --version    Prints version information
    -v, --verbose    Verbose mode (-v, -vv, -vvv)

OPTIONS:
    -O, --output <output>      Output file, stdout if not present
    -j, --threads <threads>    Number of threads [default: 40]

ARGS:
    <input>    Input file or URL (index.html, https://example.com/path/)
```

## Alternatives

* [inliner](https://github.com/remy/inliner/)
* [monolith](https://github.com/Y2Z/monolith)

## License

MIT/Unlicensed
