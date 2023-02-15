import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/common/domain/model.dart';

class TradeValues {
  Amount margin;
  double quantity;
  Leverage leverage;
  Direction direction;

  double price;
  double liquidationPrice;
  Amount fee;
  double fundingRate;

  TradeValues(
      {required this.direction,
      required this.margin,
      required this.quantity,
      required this.leverage,
      required this.price,
      required this.liquidationPrice,
      required this.fee,
      required this.fundingRate});

  factory TradeValues.create(
      {required double quantity,
      required Leverage leverage,
      required double price,
      required double fundingRate,
      required Direction direction}) {
    Amount margin = Amount(
        rust.api.calculateMargin(price: price, quantity: quantity, leverage: leverage.leverage));
    double liquidationPrice = rust.api.calculateLiquidationPrice(
        price: price, leverage: leverage.leverage, direction: direction.toApi());

    // TODO: Calculate fee based on price, quantity and funding rate
    Amount fee = Amount(30);

    return TradeValues(
        direction: direction,
        margin: margin,
        quantity: quantity,
        leverage: leverage,
        price: price,
        fundingRate: fundingRate,
        liquidationPrice: liquidationPrice,
        fee: fee);
  }

  updateQuantity(double quantity) {
    this.quantity = quantity;
    _recalculateMargin();
  }

  updateMargin(Amount margin) {
    this.margin = margin;
    _recalculateQuantity();
  }

  updatePrice(double price) {
    this.price = price;
    _recalculateMargin();
    _recalculateLiquidationPrice();
  }

  updateLeverage(Leverage leverage) {
    this.leverage = leverage;
    _recalculateMargin();
    _recalculateLiquidationPrice();
  }

  _recalculateMargin() {
    Amount margin = Amount(
        rust.api.calculateMargin(price: price, quantity: quantity, leverage: leverage.leverage));
    this.margin = margin;
  }

  _recalculateQuantity() {
    double quantity =
        rust.api.calculateQuantity(price: price, margin: margin.sats, leverage: leverage.leverage);
    this.quantity = quantity;
  }

  _recalculateLiquidationPrice() {
    double liquidationPrice = rust.api.calculateLiquidationPrice(
        price: price, leverage: leverage.leverage, direction: direction.toApi());
    this.liquidationPrice = liquidationPrice;
  }
}
