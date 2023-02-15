enum ContractSymbol { btcusd }

extension ContractSymbolExtension on ContractSymbol {
  String get label => "${name.substring(0, 3).toUpperCase()}/${name.substring(3).toUpperCase()}";
}
