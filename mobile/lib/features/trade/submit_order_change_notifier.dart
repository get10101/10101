import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/channel_opening_params.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/logger/logger.dart';

class SubmitOrderChangeNotifier extends ChangeNotifier {
  final OrderService orderService;

  SubmitOrderChangeNotifier(this.orderService);

  Future<void> submitOrder(TradeValues tradeValues,
      {ChannelOpeningParams? channelOpeningParams}) async {
    try {
      if (channelOpeningParams != null) {
        // TODO(holzeis): The coordinator leverage should not be hard coded here.
        final coordinatorCollateral = tradeValues.calculateMargin(Leverage(2.0));

        final coordinatorReserve =
            max(0, channelOpeningParams.coordinatorReserve.sub(coordinatorCollateral).sats);
        final traderReserve =
            max(0, channelOpeningParams.traderReserve.sub(tradeValues.margin!).sats);

        await orderService.submitChannelOpeningMarketOrder(
            tradeValues.leverage,
            tradeValues.contracts,
            ContractSymbol.btcusd,
            tradeValues.direction,
            false,
            Amount(coordinatorReserve),
            Amount(traderReserve));
      } else {
        await orderService.submitMarketOrder(tradeValues.leverage, tradeValues.contracts,
            ContractSymbol.btcusd, tradeValues.direction, false);
      }
    } on FfiException catch (exception) {
      logger.e("Failed to submit order: $exception");
    }
  }

  Future<ExternalFunding> submitUnfundedOrder(
      TradeValues tradeValues, ChannelOpeningParams channelOpeningParams) async {
    try {
      // TODO(holzeis): The coordinator leverage should not be hard coded here.
      final coordinatorCollateral = tradeValues.calculateMargin(Leverage(2.0));

      final coordinatorReserve =
          max(0, channelOpeningParams.coordinatorReserve.sub(coordinatorCollateral).sats);
      final traderReserve =
          max(0, channelOpeningParams.traderReserve.sub(tradeValues.margin!).sats);

      return orderService.submitUnfundedChannelOpeningMarketOrder(
        tradeValues.leverage,
        tradeValues.contracts,
        ContractSymbol.btcusd,
        tradeValues.direction,
        false,
        Amount(coordinatorReserve),
        Amount(traderReserve),
        tradeValues.margin!,
      );
    } on FfiException catch (exception) {
      logger.e("Failed to submit order: $exception");
      rethrow;
    }
  }

  Future<void> closePosition(Position position, double? closingPrice, Amount? fee) async {
    final tradeValues = TradeValues(
        direction: position.direction.opposite(),
        margin: position.collateral,
        quantity: position.quantity,
        leverage: position.leverage,
        price: closingPrice,
        liquidationPrice: position.liquidationPrice,
        fee: fee,
        expiry: position.expiry,
        tradeValuesService: const TradeValuesService());
    tradeValues.contracts = position.quantity;
    await submitOrder(tradeValues);
  }
}
