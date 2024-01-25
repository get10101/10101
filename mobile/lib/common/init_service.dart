import 'package:flutter/material.dart';
import 'package:get_10101/common/application/lsp_change_notifier.dart';
import 'package:get_10101/common/domain/lsp_config.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
import 'package:get_10101/common/collab_revert_change_notifier.dart';
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/common/recover_dlc_change_notifier.dart';
import 'package:get_10101/features/wallet/application/faucet_service.dart';
import 'package:get_10101/features/trade/rollover_change_notifier.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/send/payment_sent_change_notifier.dart';
import 'package:get_10101/features/wallet/payment_claimed_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/application/candlestick_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/async_order_change_notifier.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/domain/service_status.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/domain/price.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/environment.dart';
import 'package:nested/nested.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

List<SingleChildWidget> createProviders() {
  bridge.Config config = Environment.parse();

  const ChannelInfoService channelInfoService = ChannelInfoService();
  var tradeValuesService = TradeValuesService();

  var providers = [
    ChangeNotifierProvider(create: (context) {
      return TradeValuesChangeNotifier(tradeValuesService);
    }),
    ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
    ChangeNotifierProvider(create: (context) => SubmitOrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => OrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => PositionChangeNotifier(PositionService())),
    ChangeNotifierProvider(create: (context) => WalletChangeNotifier(const WalletService())),
    ChangeNotifierProvider(
        create: (context) => CandlestickChangeNotifier(const CandlestickService()).initialize()),
    ChangeNotifierProvider(create: (context) => ServiceStatusNotifier()),
    ChangeNotifierProvider(create: (context) => ChannelStatusNotifier()),
    ChangeNotifierProvider(create: (context) => AsyncOrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => RolloverChangeNotifier()),
    ChangeNotifierProvider(create: (context) => RecoverDlcChangeNotifier()),
    ChangeNotifierProvider(create: (context) => PaymentClaimedChangeNotifier()),
    ChangeNotifierProvider(create: (context) => PaymentChangeNotifier()),
    ChangeNotifierProvider(create: (context) => CollabRevertChangeNotifier()),
    ChangeNotifierProvider(create: (context) => LspChangeNotifier(channelInfoService)),
    Provider(create: (context) => config),
    Provider(create: (context) => channelInfoService)
  ];
  if (config.network == "regtest") {
    providers.add(Provider(create: (context) => FaucetService()));
  }

  return providers;
}

/// Forward the events from change notifiers to the Event service
void subscribeToNotifiers(BuildContext context) {
  final EventService eventService = EventService.create();

  final orderChangeNotifier = context.read<OrderChangeNotifier>();
  final positionChangeNotifier = context.read<PositionChangeNotifier>();
  final walletChangeNotifier = context.read<WalletChangeNotifier>();
  final tradeValuesChangeNotifier = context.read<TradeValuesChangeNotifier>();
  final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();
  final serviceStatusNotifier = context.read<ServiceStatusNotifier>();
  final channelStatusNotifier = context.read<ChannelStatusNotifier>();
  final asyncOrderChangeNotifier = context.read<AsyncOrderChangeNotifier>();
  final rolloverChangeNotifier = context.read<RolloverChangeNotifier>();
  final recoverDlcChangeNotifier = context.read<RecoverDlcChangeNotifier>();
  final paymentClaimedChangeNotifier = context.read<PaymentClaimedChangeNotifier>();
  final paymentChangeNotifier = context.read<PaymentChangeNotifier>();
  final collabRevertChangeNotifier = context.read<CollabRevertChangeNotifier>();
  final lspConfigChangeNotifier = context.read<LspChangeNotifier>();

  eventService.subscribe(
      orderChangeNotifier, bridge.Event.orderUpdateNotification(Order.apiDummy()));

  eventService.subscribe(
      submitOrderChangeNotifier, bridge.Event.orderUpdateNotification(Order.apiDummy()));

  eventService.subscribe(
      positionChangeNotifier, bridge.Event.positionUpdateNotification(Position.apiDummy()));

  eventService.subscribe(
      positionChangeNotifier,
      const bridge.Event.positionClosedNotification(
          bridge.PositionClosed(contractSymbol: bridge.ContractSymbol.BtcUsd)));

  eventService.subscribe(
      walletChangeNotifier, bridge.Event.walletInfoUpdateNotification(WalletInfo.apiDummy()));

  eventService.subscribe(
      tradeValuesChangeNotifier, bridge.Event.priceUpdateNotification(Price.apiDummy()));

  eventService.subscribe(
      positionChangeNotifier, bridge.Event.priceUpdateNotification(Price.apiDummy()));

  eventService.subscribe(
      serviceStatusNotifier, bridge.Event.serviceHealthUpdate(serviceUpdateApiDummy()));

  eventService.subscribe(
      asyncOrderChangeNotifier, bridge.Event.orderUpdateNotification(Order.apiDummy()));
  eventService.subscribe(
      asyncOrderChangeNotifier, bridge.Event.backgroundNotification(AsyncTrade.apiDummy()));

  eventService.subscribe(
      rolloverChangeNotifier, bridge.Event.backgroundNotification(Rollover.apiDummy()));

  eventService.subscribe(
      recoverDlcChangeNotifier, bridge.Event.backgroundNotification(RecoverDlc.apiDummy()));

  eventService.subscribe(
      paymentClaimedChangeNotifier, const bridge.Event.paymentClaimed(0, "dummy_invoice"));
  eventService.subscribe(
      paymentClaimedChangeNotifier, bridge.Event.priceUpdateNotification(Price.apiDummy()));

  eventService.subscribe(paymentChangeNotifier, const bridge.Event.paymentSent());
  eventService.subscribe(paymentChangeNotifier, const bridge.Event.paymentFailed());

  eventService.subscribe(
      collabRevertChangeNotifier, bridge.Event.backgroundNotification(CollabRevert.apiDummy()));

  eventService.subscribe(lspConfigChangeNotifier, bridge.Event.authenticated(LspConfig.apiDummy()));

  channelStatusNotifier.subscribe(eventService);

  eventService.subscribe(
      AnonSubscriber((event) => logger.i(event.field0)), const bridge.Event.log(""));
}
