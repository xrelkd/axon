{
  name,
  version,
  lib,
  rustPlatform,
  installShellFiles,
  completions ? null,
}:

rustPlatform.buildRustPackage {
  pname = name;
  inherit version;

  src = lib.cleanSource ./..;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [
    installShellFiles
  ];

  doCheck = false;

  postInstall =
    if completions != null then
      ''
        mkdir -p $out/share
        cp -r ${completions}/share/* $out/share/
      ''
    else
      ''
        installShellCompletion --cmd axon \
          --bash <($out/bin/axon completions bash) \
          --fish <($out/bin/axon completions fish) \
          --zsh  <($out/bin/axon completions zsh)
      '';

  meta = with lib; {
    description = "Axon - Command-line tool designed to simplify your interactions with Kubernetes (statically linked)";
    homepage = "https://github.com/xrelkd/axon";
    license = with licenses; [
      mit
      asl20
    ];
    platforms = platforms.linux;
    maintainers = with maintainers; [ xrelkd ];
    mainProgram = "axon";
  };
}
