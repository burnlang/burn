# A bytecode library used from Burn

`geometry.bn` is compiled into a bvm library and `app.bn` imports it.

```sh
burnc geometry.bn --target bvm -o geometry.bvmc   # build the library
burni app.bn                                       # run on bvm
burnc app.bn -o app && ./app                       # native executable with the library embedded
burnc app.bn --target bar -o app.bar && ./app.bar  # one archive with both modules
```

The library calls `hostName()`, which it declares with `@Native` and the app provides with
`@Export`. The app's `@Inject` mixin rewrites the library's `describe` function, both on bvm and in
the native executable.
