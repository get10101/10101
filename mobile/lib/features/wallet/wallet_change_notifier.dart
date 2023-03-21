import 'package:f_logs/f_logs.dart';
import 'dart:developer';
import 'package:flutter/material.dart' hide Flow;
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/payment_flow.dart';
import 'package:get_10101/features/wallet/domain/wallet_balances.dart';
import 'package:get_10101/features/wallet/domain/wallet_history.dart';
import 'domain/wallet_info.dart';

class WalletChangeNotifier extends ChangeNotifier implements Subscriber {
  final WalletService _service;
  WalletInfo walletInfo = WalletInfo(
    balances: WalletBalances(onChain: Amount(0), lightning: Amount(100)),
    // TODO: Remove this dummy data
    history: [
      WalletHistoryItemData(
          flow: PaymentFlow.inbound,
          amount: Amount(123471637),
          type: WalletHistoryItemDataType.onChain,
          status: WalletHistoryStatus.confirmed,
          timestamp: DateTime.now(),
          txid: "txidad;ofiasbdfabdfuaisdfalsdufbasdiufb"),
      WalletHistoryItemData(
          flow: PaymentFlow.inbound,
          amount: Amount(12471637),
          type: WalletHistoryItemDataType.onChain,
          status: WalletHistoryStatus.pending,
          timestamp: DateTime.now(),
          txid: "txidad;ofiasbdfabdfuaisdfalsdufbasdiufb"),
      WalletHistoryItemData(
          flow: PaymentFlow.outbound,
          amount: Amount(1000),
          type: WalletHistoryItemDataType.trade,
          status: WalletHistoryStatus.confirmed,
          timestamp: DateTime.now(),
          orderId: "123asdga7s8dasdofiasbdfabdfuaisdfalsdufbasdiufb"),
      WalletHistoryItemData(
          flow: PaymentFlow.outbound,
          amount: Amount(100000),
          type: WalletHistoryItemDataType.lightning,
          status: WalletHistoryStatus.confirmed,
          timestamp: DateTime.now(),
          nodeId: "blablayaddayaddedNodeIdIsWonderful"),
      WalletHistoryItemData(
          flow: PaymentFlow.outbound,
          amount: Amount(100000),
          type: WalletHistoryItemDataType.lightning,
          status: WalletHistoryStatus.pending,
          timestamp: DateTime.now(),
          nodeId: "blablayaddayaddedNodeIdIsWonderful")
    ],
  );

  WalletChangeNotifier(this._service);

  void update(WalletInfo? walletInfo) {
    if (walletInfo == null) {
      // skip empty wallet info update.
      return;
    }
    this.walletInfo = walletInfo;

    FLog.trace(text: 'Successfully synced payment history');
    super.notifyListeners();
  }

  Future<void> refreshWalletInfo() async {
    await _service.refreshWalletInfo();
  }

  Amount total() => Amount(onChain().sats + lightning().sats);
  Amount onChain() => walletInfo.balances.onChain;
  Amount lightning() => walletInfo.balances.lightning;

  // TODO: This is not optimal, because we map the WalletInfo in the change notifier. We can do this, but it would be better to do this on the service level.
  @override
  void notify(bridge.Event event) {
    log("Receiving this in the order notifier: ${event.toString()}");

    if (event is bridge.Event_WalletInfoUpdateNotification) {
      update(WalletInfo.fromApi(event.field0));
    } else {
      log("Received unexpected event: ${event.toString()}");
    }
  }
}
