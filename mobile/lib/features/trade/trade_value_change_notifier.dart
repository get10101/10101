import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

import 'domain/trade_values.dart';

class TradeValuesChangeNotifier extends ChangeNotifier {
  // The trade values are represented as Order domain, because that's essentially what they are
  final TradeValues _buyTradeValues = _initOrder(Direction.long);
  final TradeValues _sellTradeValues = _initOrder(Direction.short);

  // TODO: Replace dummy price with price from backend
  // TODO: Get price from separate change notifier; might be able to use a proxy change notifiers
  static const double bid = 22990.0;
  static const double ask = 23010.0;

  // TODO replace dummy funding rate with funding rate from backend
  static const double fundingRateBuy = 0.003;
  static const double fundingRateSell = -0.003;

  static TradeValues _initOrder(Direction direction) {
    double defaultQuantity = 100;
    double defaultLeverage = 2;

    switch (direction) {
      case Direction.long:
        return TradeValues.create(
            quantity: defaultQuantity,
            leverage: Leverage(defaultLeverage),
            price: ask,
            fundingRate: fundingRateBuy,
            direction: direction);
      case Direction.short:
        return TradeValues.create(
            quantity: defaultQuantity,
            leverage: Leverage(defaultLeverage),
            price: bid,
            fundingRate: fundingRateSell,
            direction: direction);
    }
  }

  void updateQuantity(Direction direction, double quantity) {
    fromDirection(direction).updateQuantity(quantity);
    notifyListeners();
  }

  void updateLeverage(Direction direction, Leverage leverage) {
    fromDirection(direction).updateLeverage(leverage);
    notifyListeners();
  }

  void updateMargin(Direction direction, Amount margin) {
    fromDirection(direction).updateMargin(margin);
    notifyListeners();
  }

  TradeValues fromDirection(Direction direction) =>
      direction == Direction.long ? _buyTradeValues : _sellTradeValues;
}
