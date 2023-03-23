import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/dummy_values.dart';

/// TODO: We should be able to depict having no price from the orderbook (e.g. it's down, we're not connected to the internet, or there are no orders etc.)
class Price {
  final double bid;
  final double ask;

  Price({required this.bid, required this.ask});

  static Price fromApi(bridge.BestPrice bestPrice) {
    return Price(bid: bestPrice.bid ?? dummyBidPrice, ask: bestPrice.ask ?? dummyAskPrice);
  }

  static bridge.BestPrice apiDummy() {
    return const bridge.BestPrice(bid: dummyBidPrice, ask: dummyAskPrice);
  }
}
