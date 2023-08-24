import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/dummy_values.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/price.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';

class StableValuesChangeNotifier extends ChangeNotifier implements Subscriber {
  final TradeValuesService tradeValuesService;
  final ChannelInfoService channelInfoService;

  late final TradeValues _sellTradeValues;

  StableValuesChangeNotifier(this.tradeValuesService, this.channelInfoService) {
    _sellTradeValues = _initOrder();
  }

  TradeValues _initOrder() {
    return TradeValues.createStable(
        quantity: 1,
        leverage: Leverage(1),
        price: null,
        fundingRate: fundingRateSell,
        direction: Direction.short,
        tradeValuesService: tradeValuesService);
  }

  /// Calculates the counterparty margin based on leverage one
  int? counterpartyMargin(Direction direction) {
    return tradeValuesService
        .calculateMargin(
            price: _sellTradeValues.price,
            quantity: _sellTradeValues.quantity,
            leverage: Leverage(1))
        ?.sats;
  }

  Amount? orderMatchingFee() {
    return stableValues().fee;
  }

  void updateQuantity(double quantity) {
    stableValues().updateQuantity(quantity);
    notifyListeners();
  }

  void updateLeverage(Leverage leverage) {
    stableValues().updateLeverage(leverage);
    notifyListeners();
  }

  void updateMargin(Amount margin) {
    stableValues().updateMargin(margin);
    notifyListeners();
  }

  // Orderbook price updates both directions
  void updatePrice(Price price) {
    bool update = false;

    if (price.bid != _sellTradeValues.price) {
      _sellTradeValues.updatePriceAndMargin(price.bid);
      update = true;
    }

    if (update) {
      notifyListeners();
    }
  }

  TradeValues stableValues() => _sellTradeValues;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_PriceUpdateNotification) {
      updatePrice(Price.fromApi(event.field0));
    }
  }
}
