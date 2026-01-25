{
    description = "An extra set of tools for managing Supabase projects going beyond the possibilities of regular Supabase CLI";

    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    };

    outputs = {
        self,
        nixpkgs,
    }: let
        systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
        forAllSystems = for: nixpkgs.lib.genAttrs systems (system: for system);
    in {
        packages = forAllSystems (
            system: let
                pkgs = import nixpkgs {
                    inherit system;
                };

                cargoManifest = fromTOML (builtins.readFile ./Cargo.toml);
                cargoExcludes = cargoManifest.package.exclude or [];

                globToRegex = glob: let
                    lib = pkgs.lib;

                    g0 =
                        if lib.hasSuffix "/" glob
                        then glob + "**"
                        else glob;

                    escaped = lib.replaceStrings
                    ["\\" "." "+" "(" ")" "|" "^" "$" "{" "}" "[" "]"]
                    ["\\\\" "\\." "\\+" "\\(" "\\)" "\\|" "\\^" "\\$" "\\{" "\\}" "\\[" "\\]"]
                    g0;

                    r1 = lib.replaceStrings ["**"] [".*"] escaped;
                    r2 = lib.replaceStrings ["*"] ["[^/]*"] r1;
                    r3 = lib.replaceStrings ["?"] ["[^/]"] r2;
                in
                    "^" + r3 + "$";

                matchesAnyExclude = relPath:
                    builtins.any (
                        pat: let
                            re = globToRegex pat;
                        in
                            builtins.match re relPath != null
                    )
                    cargoExcludes;

                root = toString ./.;

                mkPatchedCratesStager = {
                    cargoToml,
                    crateHashes ? {},
                    patchGlob ? "*.patch",
                }: let
                    cargoManifest = fromTOML (builtins.readFile cargoToml);
                    patchedCrateNames =
                        (((cargoManifest.package or {}).metadata or {}).patch or {}).crates or [];
                    dependencyDefinitions = cargoManifest.dependencies or {};

                    dependencyVersionFor = crateName: let
                        dependencyRaw =
                            if dependencyDefinitions ? ${crateName}
                            then dependencyDefinitions.${crateName}
                            else null;
                    in
                        if builtins.isString dependencyRaw
                        then dependencyRaw
                        else dependencyRaw.version;

                    patchDestinationPathFor = crateName:
                        (cargoManifest.patch."crates-io".${crateName}).path;

                    fetchUpstreamSourceFor = crateName:
                        pkgs.fetchCrate {
                            pname = crateName;
                            version = dependencyVersionFor crateName;
                            hash = crateHashes.${crateName} or pkgs.lib.fakeHash;
                        };

                    mkPatchedSourceFor = crateName: let
                        upstreamSource = fetchUpstreamSourceFor crateName;
                        patchDirectory = patchDestinationPathFor crateName;
                    in
                        pkgs.stdenvNoCC.mkDerivation {
                            name = "patched-${crateName}";
                            nativeBuildInputs = [pkgs.git];
                            unpackPhase = ''
                                cp -R ${upstreamSource}/* .
                                chmod -R u+w .
                            '';
                            buildPhase = ''
                                shopt -s nullglob
                                for patchFile in ${patchDirectory}/${patchGlob}; do
                                  git apply --unsafe-paths "$patchFile"
                                done
                            '';
                            installPhase = ''
                                mkdir -p $out
                                cp -R . $out/
                            '';
                        };

                    patchedCrates = builtins.listToAttrs
                    (map (crateName: {
                        name = crateName;
                        value = mkPatchedSourceFor crateName;
                    })
                    patchedCrateNames);

                    stageHook = pkgs.lib.concatStringsSep "\n"
                    (map (
                        crateName: let
                            destinationPath = patchDestinationPathFor crateName;
                        in ''
                            rm -rf "${destinationPath}"
                            mkdir -p "$(dirname "${destinationPath}")"
                            cp -R "${patchedCrates.${crateName}}" "${destinationPath}"
                        ''
                    )
                    patchedCrateNames);
                in {
                    inherit stageHook;
                };

                patched = mkPatchedCratesStager {
                    cargoToml = ./Cargo.toml;
                    crateHashes = {
                        "promptuity" = "sha256-385Oe4S0Kqo/xAbE7D9DmVIyKdDUQ0E72TfgU20JJns=";
                        "throbberous" = "sha256-THloggmLgP/UXkrrgfxF8BgHYNa14gsG2updbbUK+V0=";
                    };
                };
            in {
                default = pkgs.rustPlatform.buildRustPackage {
                    pname = "sbp";
                    version = cargoManifest.package.version;
                    src = pkgs.lib.cleanSourceWith {
                        src = ./.;
                        filter = path: type: let
                            p = toString path;
                            rel =
                                if pkgs.lib.hasPrefix (root + "/") p
                                then pkgs.lib.removePrefix (root + "/") p
                                else p;
                        in
                            !(matchesAnyExclude rel);
                    };

                    doCheck = false;

                    nativeBuildInputs = [pkgs.rustc pkgs.cargo];
                    cargoLock.lockFile = ./Cargo.lock;

                    buildPhase = ''
                        ${patched.stageHook}
                        cargo build --release
                    '';

                    installPhase = ''
                        mkdir -p $out/bin
                        cp -r target/release/sbp $out/bin
                    '';
                };
            }
        );

        devShells = forAllSystems (
            system: let
                pkgs = import nixpkgs {
                    inherit system;
                };
            in {
                default = pkgs.mkShell {
                    packages = [
                        pkgs.supabase-cli
                    ];
                };
            }
        );
    };
}
