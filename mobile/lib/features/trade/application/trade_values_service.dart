import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/ffi.dart' as rust;

class TradeValuesService {
  Amount? calculateMargin(
      {required double? price,
      required Amount? quantity,
      required Leverage leverage,
      dynamic hint}) {
    if (price == null || quantity == null) {
      return null;
    } else {
      return Amount(rust.api.calculateMargin(
          price: price, quantity: quantity.asDouble(), leverage: leverage.leverage));
    }
  }

  Amount? calculateQuantity(
      {required double? price, required Amount? margin, required Leverage leverage, dynamic hint}) {
    if (price == null || margin == null) {
      return null;
    } else {
      final quantity = rust.api
          .calculateQuantity(price: price, margin: margin.sats, leverage: leverage.leverage);
      return Amount(quantity.ceil());
    }
  }

  double? calculateLiquidationPrice(
      {required double? price,
      required Leverage leverage,
      required Direction direction,
      dynamic hint}) {
    if (price == null) {
      return null;
    } else {
      return rust.api.calculateLiquidationPrice(
          price: price, leverage: leverage.leverage, direction: direction.toApi());
    }
  }

  DateTime getExpiryTimestamp() {
    String network = const String.fromEnvironment('NETWORK', defaultValue: "regtest");
    return DateTime.fromMillisecondsSinceEpoch(
        rust.api.getExpiryTimestamp(network: network) * 1000);
  }
}
