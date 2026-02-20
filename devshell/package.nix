{
  name,
  version,
  lib,
  stdenv,
  rustPlatform,
  installShellFiles,
  darwin,
}:

rustPlatform.buildRustPackage {
  pname = name;
  inherit version;

  src = lib.cleanSource ./..;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  buildInputs = lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.Cocoa
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];

  nativeBuildInputs = [
    installShellFiles
  ];

  postInstall = ''
    cmd="axon"
    installShellCompletion --cmd $cmd \
      --bash <($out/bin/$cmd completions bash) \
      --fish <($out/bin/$cmd completions fish) \
      --zsh  <($out/bin/$cmd completions zsh)
  '';

  meta = with lib; {
    description = "Axon - Command-line tool designed to simplify your interactions with Kubernetes";
    homepage = "https://github.com/xrelkd/axon";
    license = with licenses; [
      mit
      asl20
    ];
    platforms = platforms.linux ++ platforms.darwin;
    maintainers = with maintainers; [ xrelkd ];
    mainProgram = "axon";
  };
}
