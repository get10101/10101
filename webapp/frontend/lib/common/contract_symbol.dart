enum ContractSymbol {
  btcusd;

  String get label => "${name.substring(0, 3).toUpperCase()}/${name.substring(3).toUpperCase()}";
}
