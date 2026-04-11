{
  name,
  version,
  axon-static,
  target ? "x86_64-unknown-linux-musl",
  runCommand,
}:

runCommand "${name}-${version}-${target}.tar.gz" { } ''
  mkdir -p $out
  tar czvf $out/${name}-${version}-${target}.tar.gz \
    -C ${axon-static} \
    bin share
''
