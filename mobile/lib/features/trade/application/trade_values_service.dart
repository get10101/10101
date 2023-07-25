import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/direction.dart';

class TradeValuesService {
  Amount? calculateMargin(
      {required double? price,
      required double? quantity,
      required Leverage leverage,
      dynamic hint}) {
    if (price == null || quantity == null) {
      return null;
    } else {
      return Amount(
          rust.api.calculateMargin(price: price, quantity: quantity, leverage: leverage.leverage));
    }
  }

  double? calculateQuantity(
      {required double? price, required Amount? margin, required Leverage leverage, dynamic hint}) {
    if (price == null || margin == null) {
      return null;
    } else {
      return rust.api
          .calculateQuantity(price: price, margin: margin.sats, leverage: leverage.leverage);
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

  Amount? orderMatchingFee({required double? price, required double? quantity, dynamic hint}) =>
      quantity != null && price != null
          ? Amount(rust.api.orderMatchingFee(quantity: quantity, price: price))
          : null;
}
