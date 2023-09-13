import 'dart:async';

import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart' hide Flow;
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/wallet_balances.dart';
import 'domain/wallet_info.dart';

class WalletChangeNotifier extends ChangeNotifier implements Subscriber {
  final WalletService service;
  late final Timer timer;

  WalletInfo walletInfo = WalletInfo(
    balances: WalletBalances(onChain: Amount(0), lightning: Amount(0)),
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

    FLog.trace(text: 'Successfully synced payment history');
    super.notifyListeners();
  }

  Future<void> initialize() async {
    await refreshWalletInfo();

    timer = Timer.periodic(const Duration(seconds: 30), (Timer t) async {
      await refreshWalletInfo();
    });
  }

  Future<void> refreshWalletInfo() async {
    syncing = true;
    await service.refreshWalletInfo();
    syncing = false;
  }

  Amount total() => Amount(onChain().sats + lightning().sats);
  Amount onChain() => walletInfo.balances.onChain;
  Amount lightning() => walletInfo.balances.lightning;

  // TODO: This is not optimal, because we map the WalletInfo in the change notifier. We can do this, but it would be better to do this on the service level.
  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_WalletInfoUpdateNotification) {
      update(WalletInfo.fromApi(event.field0));
    } else {
      FLog.warning(text: "Received unexpected event: ${event.toString()}");
    }
  }
}
