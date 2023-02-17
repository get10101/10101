import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/features/trade/domain/leverage.dart';
import '../../../common/domain/model.dart';
import '../domain/direction.dart';

class TradeValuesService {
  Amount calculateMargin(
      {required double price, required double quantity, required Leverage leverage, dynamic hint}) {
    return Amount(
        rust.api.calculateMargin(price: price, quantity: quantity, leverage: leverage.leverage));
  }

  double calculateQuantity(
      {required double price, required Amount margin, required Leverage leverage, dynamic hint}) {
    return rust.api
        .calculateQuantity(price: price, margin: margin.sats, leverage: leverage.leverage);
  }

  double calculateLiquidationPrice(
      {required double price,
      required Leverage leverage,
      required Direction direction,
      dynamic hint}) {
    return rust.api.calculateLiquidationPrice(
        price: price, leverage: leverage.leverage, direction: direction.toApi());
  }
}
