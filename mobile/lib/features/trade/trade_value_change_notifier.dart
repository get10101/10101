import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/dummy_values.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/price.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';

class TradeValuesChangeNotifier extends ChangeNotifier implements Subscriber {
  final TradeValuesService tradeValuesService;

  // The trade values are represented as Order domain, because that's essentially what they are
  late final TradeValues _buyTradeValues;
  late final TradeValues _sellTradeValues;

  Price? _price;

  TradeValuesChangeNotifier(this.tradeValuesService) {
    _buyTradeValues = _initOrder(Direction.long);
    _sellTradeValues = _initOrder(Direction.short);
  }

  TradeValues _initOrder(Direction direction) {
    Usd defaultQuantity = Usd(500);
    Leverage defaultLeverage = Leverage(2);

    switch (direction) {
      case Direction.long:
        return TradeValues.fromQuantity(
            quantity: defaultQuantity,
            leverage: defaultLeverage,
            price: null,
            fundingRate: fundingRateBuy,
            direction: direction,
            tradeValuesService: tradeValuesService,
            isMarginOrder: false);
      case Direction.short:
        return TradeValues.fromQuantity(
            quantity: defaultQuantity,
            leverage: defaultLeverage,
            price: null,
            fundingRate: fundingRateSell,
            direction: direction,
            tradeValuesService: tradeValuesService,
            isMarginOrder: false);
    }
  }

  /// Calculates the counterparty margin
  int? counterpartyMargin(Direction direction, double leverage, double price, Usd quantity) {
    switch (direction) {
      case Direction.long:
        return tradeValuesService
            .calculateMargin(price: price, quantity: quantity, leverage: Leverage(leverage))
            ?.sats;
      case Direction.short:
        return tradeValuesService
            .calculateMargin(price: price, quantity: quantity, leverage: Leverage(leverage))
            ?.sats;
    }
  }

  Amount? orderMatchingFee(Direction direction) {
    return fromDirection(direction).fee;
  }

  void updateQuantity(Direction direction, Usd quantity) {
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

  // Orderbook price updates both directions
  void updatePrice(Price price) {
    bool update = false;

    if (price.ask != _buyTradeValues.price) {
      _buyTradeValues.updatePriceAndMargin(price.ask);
      update = true;
    }
    if (price.bid != _sellTradeValues.price) {
      _sellTradeValues.updatePriceAndMargin(price.bid);
      update = true;
    }
    _price = price;

    if (update) {
      notifyListeners();
    }
  }

  void updateIsMargin(Direction direction, bool isMargin) {
    fromDirection(direction).updateIsMargin(isMargin);
    notifyListeners();
  }

  Price? getPrice() {
    return _price;
  }

  TradeValues fromDirection(Direction direction) =>
      direction == Direction.long ? _buyTradeValues : _sellTradeValues;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_PriceUpdateNotification) {
      updatePrice(Price.fromApi(event.field0));
    }
  }
}
