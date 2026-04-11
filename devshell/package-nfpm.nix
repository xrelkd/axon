{
  name,
  version,
  lib,
  nfpm,
  axon-static,
  pkgs,
  stdenv,
  packager ? "deb",
  arch ? "amd64",
}:

let
  nfpmConfig = pkgs.replaceVars ./nfpm.yaml {
    NAME = name;
    VERSION = version;
    ARCH = arch;
  };
in
stdenv.mkDerivation {
  pname = "${name}-${packager}";
  inherit version;

  nativeBuildInputs = [ nfpm ];

  dontUnpack = true;
  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall

    staging=$(mktemp -d)
    mkdir -p "$staging/usr/bin"

    cp ${axon-static}/bin/axon "$staging/usr/bin/"

    mkdir -p $out
    cd "$staging"
    nfpm package -f ${nfpmConfig} --packager ${packager} --target "$out"

    runHook postInstall
  '';

  meta = with lib; {
    description = "Axon - Command-line tool (statically linked, ${packager} package)";
    homepage = "https://github.com/xrelkd/axon";
    license = with licenses; [
      mit
      asl20
    ];
    platforms = platforms.linux;
    maintainers = with maintainers; [ xrelkd ];
  };
}
