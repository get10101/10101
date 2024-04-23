import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

class TradeValues {
  /// Potential quantity already in an open position
  ///
  /// Note the open quantity is only set for the opposite direction.
  /// So if you'd go 100 long the open quantity would be 0 for the long direction and 100 for the
  /// short direction.
  Usd _openQuantity = Usd.zero();

  get openQuantity => _openQuantity;

  set openQuantity(quantity) => _openQuantity = quantity;

  /// The difference between open quantity and quantity if contracts is bigger than the open quantity.
  /// This value is used to calculate the required margin.
  Usd quantity = Usd.zero();

  /// The actual contracts entered. Any value between 0 and maxQuantity.
  Usd contracts = Usd.zero();

  Amount? margin;
  Leverage leverage;
  Direction direction;

  double? price;
  double? liquidationPrice;
  Amount? fee; // This fee is an estimate of the order-matching fee.

  Usd? _maxQuantity;

  /// The max quantity of the contracts not part of the open quantity.
  Usd get maxQuantity => _maxQuantity ?? Usd.zero();

  DateTime expiry;

  // no final so it can be mocked in tests
  TradeValuesService tradeValuesService;

  TradeValues({
    required this.direction,
    required this.margin,
    required this.quantity,
    required this.leverage,
    required this.price,
    required this.liquidationPrice,
    required this.fee,
    required this.expiry,
    required this.tradeValuesService,
  });

  factory TradeValues.fromQuantity({
    required Usd quantity,
    required Leverage leverage,
    required double? price,
    required Direction direction,
    required TradeValuesService tradeValuesService,
  }) {
    Amount? margin =
        tradeValuesService.calculateMargin(price: price, quantity: quantity, leverage: leverage);

    double? liquidationPrice = tradeValuesService.calculateLiquidationPrice(
        price: price, leverage: leverage, direction: direction);

    Amount? fee = tradeValuesService.orderMatchingFee(quantity: quantity, price: price);

    DateTime expiry = tradeValuesService.getExpiryTimestamp();

    return TradeValues(
        direction: direction,
        margin: margin,
        quantity: quantity,
        leverage: leverage,
        price: price,
        liquidationPrice: liquidationPrice,
        fee: fee,
        expiry: expiry,
        tradeValuesService: tradeValuesService);
  }

  updateQuantity(Usd quantity) {
    this.quantity = quantity;
    _recalculateMargin();
  }

  updateContracts(Usd contracts) {
    this.contracts = contracts;
    _recalculateMargin();
    _recalculateFee();
    _recalculateLiquidationPrice();
  }

  updateMargin(Amount margin) {
    this.margin = margin;
    _recalculateQuantity();
    _recalculateFee();
    recalculateMaxQuantity();
  }

  updatePriceAndQuantity(double? price) {
    this.price = price;
    _recalculateQuantity();
    _recalculateLiquidationPrice();
    _recalculateFee();
    recalculateMaxQuantity();
  }

  updatePriceAndMargin(double? price) {
    this.price = price;
    _recalculateMargin();
    _recalculateLiquidationPrice();
    _recalculateFee();
    recalculateMaxQuantity();
  }

  updateLeverage(Leverage leverage) {
    this.leverage = leverage;
    _recalculateMargin();
    _recalculateLiquidationPrice();
    recalculateMaxQuantity();
  }

  // Can be used to calculate the counterparty's margin, based on their
  // leverage.
  calculateMargin(Leverage leverage) {
    return tradeValuesService.calculateMargin(price: price, quantity: quantity, leverage: leverage);
  }

  _recalculateMargin() {
    Amount? margin =
        tradeValuesService.calculateMargin(price: price, quantity: quantity, leverage: leverage);
    this.margin = margin;
  }

  _recalculateQuantity() {
    Usd? quantity =
        tradeValuesService.calculateQuantity(price: price, margin: margin, leverage: leverage);
    this.quantity = quantity ?? Usd.zero();
  }

  _recalculateLiquidationPrice() {
    if (quantity.usd == 0) {
      // the user is only reducing his position hence we need to calculate the liquidation price based on the opposite direction.
      double? liquidationPrice = tradeValuesService.calculateLiquidationPrice(
          price: price, leverage: leverage, direction: direction.opposite());
      this.liquidationPrice = liquidationPrice;
    } else {
      double? liquidationPrice = tradeValuesService.calculateLiquidationPrice(
          price: price, leverage: leverage, direction: direction);
      this.liquidationPrice = liquidationPrice;
    }
  }

  _recalculateFee() {
    fee = tradeValuesService.orderMatchingFee(quantity: contracts, price: price);
  }

  recalculateMaxQuantity() {
    final quantity = tradeValuesService.calculateMaxQuantity(
        price: price, leverage: leverage, direction: direction);
    if (quantity != null) {
      _maxQuantity = quantity;
    }
  }
}
