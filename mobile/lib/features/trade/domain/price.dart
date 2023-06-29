import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

class Price {
  final double? bid;
  final double? ask;

  Price({required this.bid, required this.ask});

  isValid() {
    return bid != null && ask != null;
  }

  static Price fromApi(bridge.BestPrice bestPrice) {
    return Price(bid: bestPrice.bid, ask: bestPrice.ask);
  }

  static bridge.BestPrice apiDummy() {
    return const bridge.BestPrice(bid: null, ask: null);
  }
}
