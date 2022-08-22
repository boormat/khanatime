# Khana Timing project

[![test](https://github.com/boormat/khanatime/actions/workflows/test.yml/badge.svg)](https://github.com/boormat/khanatime/actions/workflows/test.yml)

Live App/Web Install: https://boormat.github.io/khanatime/

Now Rust this time
Rust WASM for client side
  - Building with trunk
  - yew? or seed for stuff.
Rust Rocket for server

# Tech decisions

Rust + Seed
Seed seems pretty well documented.  Big maintenance saving is no JavaScript
errors to deal with.  When you change something the compiler will fail or warn
you of unhandle events/messages etc.  Way faster than tracing state in console.

In particular the seed-time-tracker demo goes deep into more complexity than we
need, but will follow similar goals.

Trunk for WASM build.  It just seems to work, unlike wasm-pack that had
dependency problems. Styling with SASS, Bulma library. Cargo supports SASS out
of the box. Fontawesome for icons.

Probably going be simplest and cleanest to users to have 2 different web apps,
the timer one for stages, and the results one for hq.
To make that work, could use trunk arguments to keep the 2 apps separate,
and mess with arguments.

Probably simpler is to use Cargo Workspaces(?), so each WebAsm is more normal
structure with some helper libraries, for at least the shared models.

Write it as a single WASM app, similar to the SEED time tracker.

Pages:
- home menu/event picker
- Results view.  Just render
- Scorer.  Central Data entry.  Fast keyboard mode.  Import/check of stage times.
- Timer. Time entry/stop watch for stage officials.
- Maybe assistant timer?

Data Store:
Use local storage to operate offline.  Offline sharing of documents as primary
option.  E.g. email, text, Bluetooth magic.
 - Stage times as a doc.  List of times for that stage.  Basically the primary
    (maybe consider a stage cmdr to resolve )
 - Event as a doc.  Owner by Scorer.  They import state times and/or manually
    enter from paper.
 - Published results?  Publish to web? PDF + in the json form.  Maybe a server
    for http streaming results. Also bluetooth etc for offline events.

The data flow from time entry to scorer to published results is a 3 step
process, not trying to publish results before become approved. Can consider
that later, basically http stream the updates.

Comms:  Aim to work offline.  web_sys::bluetooth

Note will need to tell trunk to use inline mode for stylesheets. data-inline

The main pages:  Initially have a stage time and results 


## 2. Install / check required tools

1. Make sure you have basic tools installed:

  - [Rust](https://www.rust-lang.org)
  - cargo install --locked trunk

Once you've installed Trunk, simply execute `trunk serve --open` from this 
example's directory, and you should see the web application rendered in your
browser.



## Testing and Deploy

Currently manual triggered release build in github workers.
https://github.com/boormat/khanatime/actions
Run the deploy workflow to update  https://boormat.github.io/khanatime/

When testing locally with trunk and chrome, the the WebWorkers seem to mess
up when the reload comes from trunk, and you get a blank screen.
A workaround is  developer tools -> Service Workers -> Update on Reload On
