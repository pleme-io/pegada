{
  description = "Pegada — the footprint a lifecycle leaves where it stood, and the typed proof that it is gone";

  inputs.substrate.url = "github:pleme-io/substrate";

  outputs =
    { substrate, ... }:
    substrate.rust.library {
      src = ./.;
    };
}
