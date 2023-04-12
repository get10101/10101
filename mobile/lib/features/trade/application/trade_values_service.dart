import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/direction.dart';

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

  int getFeeReserve() {
    // TODO: Fetch from backend
    // This hardcoded value corresponds to the fee-rate of 4 sats per vbyte. We should relate this value to that fee-rate in the backend.
    return 1666;
  }

  int getChannelReserve() {
    // TODO: Fetch from backend
    // This is the minimum value that has to remain in the channel. It is defined in rust-lightning and we should fetch this value from the corresponding constant in the backend.
    return 1000;
  }

  int getMinTradeMargin() {
    // This value is an arbitrary number; we only allow trades with a minimum of 1000 sats margin.
    return 1000;
  }
}
