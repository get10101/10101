import 'package:get_10101/ffi.dart' as rust;

enum ContractSymbol { btcusd }

extension ContractSymbolExtension on ContractSymbol {
  String get label => "${name.substring(0, 3).toUpperCase()}/${name.substring(3).toUpperCase()}";

  rust.ContractSymbol toApi() {
    switch (this) {
      case ContractSymbol.btcusd:
        return rust.ContractSymbol.BtcUsd;
    }
  }
}
