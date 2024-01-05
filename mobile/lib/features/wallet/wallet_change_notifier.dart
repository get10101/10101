import 'dart:async';

import 'package:flutter/material.dart' hide Flow;
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/wallet_balances.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';

class WalletChangeNotifier extends ChangeNotifier implements Subscriber {
  final WalletService service;
  WalletInfo walletInfo = WalletInfo(
    balances: WalletBalances(onChain: Amount(0), offChain: Amount(0)),
    history: List.empty(),
  );
  bool syncing = true;

  WalletChangeNotifier(this.service);

  void update(WalletInfo? walletInfo) {
    if (walletInfo == null) {
      // skip empty wallet info update.
      return;
    }
    this.walletInfo = walletInfo;
    syncing = false;

    logger.t('Successfully synced payment history');
    super.notifyListeners();
  }

  Future<void> refreshLightningWallet() async {
    await service.refreshLightningWallet();
  }

  Future<void> refreshWalletInfo() async {
    syncing = true;
    await service.refreshWalletInfo();
  }

  Amount total() => Amount(onChain().sats + offChain().sats);

  Amount onChain() => walletInfo.balances.onChain;

  Amount offChain() => walletInfo.balances.offChain;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_WalletInfoUpdateNotification) {
      update(WalletInfo.fromApi(event.field0));
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
    syncing = false;
  }

  Future<void> waitForSyncToComplete() async {
    final completer = Completer<void>();

    void checkSyncingStatus() {
      if (!syncing) {
        completer.complete();
      } else {
        Future.delayed(const Duration(milliseconds: 200), checkSyncingStatus);
      }
    }

    checkSyncingStatus();

    await completer.future;
  }
}
