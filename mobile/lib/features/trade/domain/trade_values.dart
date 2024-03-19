import 'package:flutter/widgets.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:intl/intl.dart';

class TradeValues {
  Amount? margin;
  Leverage leverage;
  Direction direction;

  // These values  can be null if coordinator is down
  Usd? quantity;
  double? price;
  double? liquidationPrice;
  Amount? fee; // This fee is an estimate of the order-matching fee.

  double fundingRate;
  DateTime expiry;

  // no final so it can be mocked in tests
  TradeValuesService tradeValuesService;

  bool isMarginOrder;

  final TextEditingController marginController;
  final TextEditingController quantityController;
  final TextEditingController priceController;

  TradeValues({
    required this.direction,
    required this.margin,
    required this.quantity,
    required this.leverage,
    required this.price,
    required this.liquidationPrice,
    required this.fee,
    required this.fundingRate,
    required this.expiry,
    required this.tradeValuesService,
    required this.isMarginOrder,
    required this.quantityController,
    required this.marginController,
    required this.priceController,
  });

  factory TradeValues.fromQuantity(
      {required Usd quantity,
      required Leverage leverage,
      required double? price,
      required double fundingRate,
      required Direction direction,
      required TradeValuesService tradeValuesService,
      required bool isMarginOrder}) {
    Amount? margin =
        tradeValuesService.calculateMargin(price: price, quantity: quantity, leverage: leverage);
    double? liquidationPrice = price != null
        ? tradeValuesService.calculateLiquidationPrice(
            price: price, leverage: leverage, direction: direction)
        : null;

    Amount? fee = tradeValuesService.orderMatchingFee(quantity: quantity, price: price);

    DateTime expiry = tradeValuesService.getExpiryTimestamp();

    TextEditingController marginController = TextEditingController();
    TextEditingController quantityController = TextEditingController();
    TextEditingController priceController = TextEditingController();

    if (isMarginOrder) {
      marginController.text = margin?.formatted() ?? "0.0";
      quantityController.text = "~${quantity.formatted()}";
    } else {
      marginController.text = "~${margin?.formatted() ?? 0.0}";
      final formatter = NumberFormat("#,###,###,###,###", "en");
      quantityController.text = formatter.format(quantity.asDouble());
    }

    return TradeValues(
        direction: direction,
        margin: margin,
        quantity: quantity,
        leverage: leverage,
        price: price,
        fundingRate: fundingRate,
        liquidationPrice: liquidationPrice,
        fee: fee,
        expiry: expiry,
        tradeValuesService: tradeValuesService,
        isMarginOrder: isMarginOrder,
        quantityController: quantityController,
        marginController: marginController,
        priceController: priceController);
  }

  updateIsMargin(bool isMargin) {
    isMarginOrder = isMargin;
    _recalculateQuantity();
    _recalculateLiquidationPrice();
    _recalculateFee();
  }

  updatePrice(double newPrice) {
    price = newPrice;
    if (isMarginOrder) {
      _recalculateQuantity();
    } else {
      _recalculateMargin();
    }
    _recalculateLiquidationPrice();
    _recalculateFee();
  }

  updateQuantity(Usd quantity) {
    this.quantity = quantity;
    _recalculateMargin();
    _recalculateFee();
  }

  updateMargin(Amount margin) {
    this.margin = margin;
    _recalculateQuantity();
    _recalculateFee();
  }

  updatePriceAndQuantity(double? price) {
    this.price = price;
    _recalculateQuantity();
    _recalculateLiquidationPrice();
    _recalculateFee();
  }

  updatePriceAndMargin(double? price) {
    this.price = price;
    _recalculateMargin();
    _recalculateLiquidationPrice();
    _recalculateFee();
  }

  updateLeverage(Leverage leverage) {
    this.leverage = leverage;
    if (isMarginOrder) {
      _recalculateQuantity();
    } else {
      _recalculateMargin();
    }
    _recalculateLiquidationPrice();
    _recalculateFee();
  }

  // Can be used to calculate the counterparty's margin, based on their
  // leverage.
  calculateMargin(Leverage leverage) {
    return tradeValuesService.calculateMargin(price: price, quantity: quantity, leverage: leverage);
  }

  _updateMarginControllers() {
    if (isMarginOrder) {
      marginController.text = margin?.formatted() ?? "0.0";
    } else {
      marginController.text = "~${margin?.formatted() ?? 0.0}";
    }
  }

  _updateQuantityControllers() {
    if (isMarginOrder) {
      quantityController.text = "~${quantity?.formatted() ?? 0.0}";
    } else {
      final formatter = NumberFormat("#,###,###,###,###", "en");
      quantityController.text = formatter.format(quantity?.asDouble() ?? 0.0);
    }
  }

  _recalculateMargin() {
    Amount? margin =
        tradeValuesService.calculateMargin(price: price, quantity: quantity, leverage: leverage);
    this.margin = margin;
    _updateMarginControllers();
  }

  _recalculateQuantity() {
    Usd? quantity =
        tradeValuesService.calculateQuantity(price: price, margin: margin, leverage: leverage);
    this.quantity = quantity;
    _updateQuantityControllers();
  }

  _recalculateLiquidationPrice() {
    double? liquidationPrice = tradeValuesService.calculateLiquidationPrice(
        price: price, leverage: leverage, direction: direction);
    this.liquidationPrice = liquidationPrice;
  }

  _recalculateFee() {
    fee = tradeValuesService.orderMatchingFee(quantity: quantity, price: price);
  }
}
