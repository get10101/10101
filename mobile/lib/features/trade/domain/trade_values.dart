import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';

class TradeValues {
  Amount margin;
  double quantity;
  Leverage leverage;
  Direction direction;

  double price;
  double liquidationPrice;
  Amount fee;
  double fundingRate;

  // no final so it can be mocked in tests
  TradeValuesService tradeValuesService;

  TradeValues(
      {required this.direction,
      required this.margin,
      required this.quantity,
      required this.leverage,
      required this.price,
      required this.liquidationPrice,
      required this.fee,
      required this.fundingRate,
      required this.tradeValuesService});

  factory TradeValues.create(
      {required Amount margin,
      required Leverage leverage,
      required double price,
      required double fundingRate,
      required Direction direction,
      required TradeValuesService tradeValuesService}) {
    double quantity =
        tradeValuesService.calculateQuantity(price: price, margin: margin, leverage: leverage);
    double liquidationPrice = tradeValuesService.calculateLiquidationPrice(
        price: price, leverage: leverage, direction: direction);

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
        fee: fee,
        tradeValuesService: tradeValuesService);
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
    _recalculateQuantity();
    _recalculateLiquidationPrice();
  }

  updateLeverage(Leverage leverage) {
    this.leverage = leverage;
    _recalculateMargin();
    _recalculateLiquidationPrice();
  }

  _recalculateMargin() {
    Amount margin =
        tradeValuesService.calculateMargin(price: price, quantity: quantity, leverage: leverage);
    this.margin = margin;
  }

  _recalculateQuantity() {
    double quantity =
        tradeValuesService.calculateQuantity(price: price, margin: margin, leverage: leverage);
    this.quantity = quantity;
  }

  _recalculateLiquidationPrice() {
    double liquidationPrice = tradeValuesService.calculateLiquidationPrice(
        price: price, leverage: leverage, direction: direction);
    this.liquidationPrice = liquidationPrice;
  }
}
