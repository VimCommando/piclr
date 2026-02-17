# The DATA Stack

This was my concept for the DATA Stack:

- **Datastar** - Event-driven hypermedia interface
- **Askama** - Compile-time HTML templating
- **Tauri** - Native desktop application wrapper
- **Axum** - Bridging HTTP interface and state management

## The Origins

In 2024 I started building a command-line interface (CLI) tool in Rust. I picked
Rust for its high performance ceiling and type safety. I didn't want a garbage
collected language, and I didn't want to be effectively locked into the Apple
ecosystem with Swift. So Rust was the last widely-used language standing.

As powerful as CLI tools are, they can struggle with broad adoption, because
they are more power-user oriented. So after a few months I started adding a web
interface on top of the CLI backend. This brought with it a major challenge: I
absolutely did _not_ want to introduce React or any other heavyweight Javascript
framework into my code base. I wanted to keep it lean and fast.

### Datastar

Datastar provides the highly responsive UI behavior without requiring a full
Javascript framework. I briefly tested the waters with HTMX, but I bounced off
it as soon as I realized almost all of my desired actions were "out of band". I
landed on [Datastar](https://data-star.dev) and it is _amazing_. I have to write so
little Javascript to get exactly the kind of interactivity I want. I will never
use React again.

Key advantages:
- Minimizes the amount of client-side Javascript with rich interactivity
- All interactions can trigger immediately from an open event stream
- The HTML _is_ the client state [HATEOAS](https://en.wikipedia.org/wiki/HATEOAS)
- Enables the command-query responsibility separation (CQRS) [design
  pattern](https://medium.com/design-microservices-architecture-with-patterns/cqrs-design-pattern-in-microservices-architectures-5d41e359768c)
- The ~10kb `datastar.js` file is a complete no-brainer to embed into the Rust binary

This means backend language independence, easy bundling, less code, no polling
and low-latency UI interaction. It can feel native when run local _and_ still be
great when hosted as a web service.

### Askama

Since Datastar works on two things, HTML elements and JSON signals, I needed
something to render the HTML from the backend. I picked Askama because:
- It uses a familiar Jinja-like syntax
- It uses compile-time rendering so there is minimal runtime overhead
- It is both mature and popular enough to have a solid community

It just happens to be in the top 5 for Rust [template engine performance](
https://github.com/askama-rs/template-benchmark?tab=readme-ov-file#benchmark-results)

The templates themselves are just partial HTML elements. It maps incredibly well
into Datastar's `PatchElements` philosophy, so I can just render a struct and
yield a Datastar event with the rendered representation.

### Tauri

Where a hosted web application offers ease of use, a desktop GUI offers power.
If the application needs to store secrets or user-specific configurations, a
shared hosted service has its own challenges. Web applications are also not the
easiest to distribute to end users unless you're running a SaaS model, which I'm
not.

So Tauri bridges this gap:
- Allows the web application to be distributed in a native executable
- Enables native capabilities otherwise sandboxed and denied to browser security
- Smaller application bundles and resource usage with no bundled web browser 

This makes one highly performant UI work across all platforms, including as a 
web service.

### Axum

To handle the API I went with Axum. Even if Actix might offer [higher peak
throughput](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2),
it requires more memory and adds complexity, so it wasn't worth it.

Axum wins:
- Mature, trusted, proven library maintained by the Tokio team
- Good developer ergonomics, easy to work with
- Full support for server-side event streams 
- No opinionated templating, working great with Askama

And of course, because Datastar's Rust SDK includes Axum support :)

## Rust

Reinforcing why I decided on a Rust backend:

1. High performance ceiling
2. Compiled and distributed with no runtime requirement
3. Simplifies and reinforces good programming practices
4. Active multi-platform community
5. No garbage collector

It's avoiding that GC that knocked out other languages like Go. For most
applications it won't matter. But when you're trying to blast through gigabytes
of raw text files to output millions of JSON documents, and you don't want your
users to wait minutes to see the results, that backend throughput matters.

Rust also works exceptionally well with the [type-state
pattern](https://cliffle.com/blog/rust-typestate/), or state machines, which can
make [invalid states
unrepresentable](https://www.youtube.com/watch?v=z-0-bbc80JM). As someone who
has never been a full-time software engineer, these type-enforced guardrails
keep both me and our new AI coding assistants safely grounded in allowed states.
