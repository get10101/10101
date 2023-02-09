import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart' hide Flow;
import 'package:get_10101/ffi.dart';

class WalletChangeNotifier extends ChangeNotifier {
  WalletInfo walletInfo = WalletInfo(
      balances: Balances(onChain: 0, lightning: 0),
      history: List.empty(),
  );

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
    try {
      final walletInfo = await api.refreshWalletInfo();
      update(walletInfo);
      FLog.trace(text: 'Successfully refreshed wallet info');
    } catch (error) {
      FLog.error(text: "Failed to get wallet info: $error");
    }
  }

  int total() => onChain() + lightning();
  int onChain() => walletInfo.balances.onChain;
  int lightning() => walletInfo.balances.lightning;
}
