enum OrderType {
  market,
  limit;

  String get asString {
    switch (this) {
      case OrderType.market:
        return "Market";
      case OrderType.limit:
        return "Limit";
    }
  }

  // Factory method to convert a String to OrderType
  static OrderType fromString(String value) {
    switch (value.toLowerCase()) {
      case 'market':
        return OrderType.market;
      case 'limit':
        return OrderType.limit;
      default:
        throw ArgumentError('Invalid OrderType: $value');
    }
  }
}
