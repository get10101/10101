import 'package:get_10101/ffi.dart' as rust;

enum ContractSymbol {
  btcusd;

  static ContractSymbol fromApi(rust.ContractSymbol contractSymbol) {
    switch (contractSymbol) {
      case rust.ContractSymbol.BtcUsd:
        return ContractSymbol.btcusd;
    }
  }

  String get label => "${name.substring(0, 3).toUpperCase()}/${name.substring(3).toUpperCase()}";

  rust.ContractSymbol toApi() {
    switch (this) {
      case ContractSymbol.btcusd:
        return rust.ContractSymbol.BtcUsd;
    }
  }
}
