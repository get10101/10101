import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';

class TradeValuesChangeNotifier extends ChangeNotifier implements Subscriber {
  final TradeValuesService tradeValuesService;

  // The trade values are represented as Order domain, because that's essentially what they are
  late final TradeValues _buyTradeValues;
  late final TradeValues _sellTradeValues;

  double? _askPrice;
  double? _bidPrice;

  bool maxQuantityLock = false;

  TradeValuesChangeNotifier(this.tradeValuesService) {
    _buyTradeValues = _initOrder(Direction.long);
    _sellTradeValues = _initOrder(Direction.short);
  }

  TradeValues _initOrder(Direction direction) {
    // the default quantity will be calculated when the trade bottom sheet tab is initialized.
    Usd defaultQuantity = Usd.zero();
    Leverage defaultLeverage = Leverage(2);

    switch (direction) {
      case Direction.long:
        return TradeValues.fromQuantity(
            quantity: defaultQuantity,
            leverage: defaultLeverage,
            price: null,
            direction: direction,
            tradeValuesService: tradeValuesService);
      case Direction.short:
        return TradeValues.fromQuantity(
            quantity: defaultQuantity,
            leverage: defaultLeverage,
            price: null,
            direction: direction,
            tradeValuesService: tradeValuesService);
    }
  }

  /// Calculates the counterparty margin based on leverage one
  int? counterpartyMargin(Direction direction, double leverage) {
    switch (direction) {
      case Direction.long:
        return tradeValuesService
            .calculateMargin(
                price: _buyTradeValues.price,
                quantity: _buyTradeValues.quantity,
                leverage: Leverage(leverage))
            ?.sats;
      case Direction.short:
        return tradeValuesService
            .calculateMargin(
                price: _sellTradeValues.price,
                quantity: _sellTradeValues.quantity,
                leverage: Leverage(leverage))
            ?.sats;
    }
  }

  Amount? orderMatchingFee(Direction direction) {
    return fromDirection(direction).fee;
  }

  void updateQuantity(Direction direction, Usd quantity) {
    if (fromDirection(direction).openQuantity < quantity) {
      // the user is changing direction of his position
      fromDirection(direction).updateQuantity(quantity - fromDirection(direction).openQuantity);
    } else {
      // the user is only selling existing contracts
      fromDirection(direction).updateQuantity(Usd.zero());
    }

    fromDirection(direction).updateContracts(quantity);

    notifyListeners();
  }

  void updateLeverage(Direction direction, Leverage leverage) {
    fromDirection(direction).updateLeverage(leverage);
    maxQuantityLock = false;
    notifyListeners();
  }

  void updateMargin(Direction direction, Amount margin) {
    fromDirection(direction).updateMargin(margin);
    notifyListeners();
  }

  void updateMaxQuantity() {
    _sellTradeValues.recalculateMaxQuantity();
    _buyTradeValues.recalculateMaxQuantity();
  }

  // Orderbook price updates both directions
  void updatePrice(double price, Direction direction) {
    bool update = false;

    switch (direction) {
      case Direction.long:
        _bidPrice = price;
        if (price != _sellTradeValues.price) {
          if (maxQuantityLock) {
            _sellTradeValues.updatePriceAndQuantity(price);
            _sellTradeValues.contracts = _sellTradeValues.maxQuantity;
          } else {
            _sellTradeValues.updatePriceAndMargin(price);
          }
          update = true;
        }
      case Direction.short:
        _askPrice = price;
        if (price != _buyTradeValues.price) {
          if (maxQuantityLock) {
            _buyTradeValues.updatePriceAndQuantity(price);
            _buyTradeValues.contracts = _buyTradeValues.maxQuantity;
          } else {
            _buyTradeValues.updatePriceAndMargin(price);
          }
          update = true;
        }
    }

    if (update) {
      notifyListeners();
    }
  }

  double? getAskPrice() {
    return _askPrice;
  }

  double? getBidPrice() {
    return _bidPrice;
  }

  TradeValues fromDirection(Direction direction) =>
      direction == Direction.long ? _buyTradeValues : _sellTradeValues;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_AskPriceUpdateNotification) {
      updatePrice(event.field0, Direction.short);
    }
    if (event is bridge.Event_BidPriceUpdateNotification) {
      updatePrice(event.field0, Direction.long);
    }
  }
}
