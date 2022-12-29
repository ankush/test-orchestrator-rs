# BlAzInGlY fast test orchestrator

At Frappe we run a lot of tests on each PR, so we need to run them in parallel and we need to run them fast! 

[@Suraj](https://github.com/surajshetty3416) created this brilliant [test orchestrator](https://github.com/frappe/test-orchestrator) in NodeJS, it just wasn't enough. It never felt enough, how can it be so simple? Also global shared state in NodeJS? That sounds like race condition bugs waiting to appear!

So I rewrote it in rust. Sure after looking at implementation you might say the mutex makes it practically synchronous but it still is blazingly fast because it's written in Rust. The numbers dont lie!


```
# Node js Version
λ time python test.py
All good
python test.py  2.18s user 0.44s system 109% cpu 2.491 total

# Rust Version
λ time python test.py
All good
python test.py  2.17s user 0.54s system 109% cpu 2.473 total
```

Ok now you might say it's only 0.01 seconds improvement but that's just because of slow test suite in python. Entire Frappe Framework needs to be rewritten in rust to truly benefit from this blazingly fast rewrite.


 <details>
  <summary>Disclaimer</summary>
  Ofcourse this is a joke. I love rust btw. 
</details> 
