import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/payment_flow.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'transaction.dart';
import 'wallet_balances.dart';

class WalletInfo {
  WalletBalances balances;
  List<Transaction> history;

  WalletInfo({required this.balances, required this.history});
  WalletInfo.fromApi(rust.WalletInfo walletInfo):
    balances = WalletBalances(
        onChain: Amount(walletInfo.balances.onChain),
        lightning: Amount(walletInfo.balances.lightning)
    ),
    history = walletInfo.history.map((tx) {
      return Transaction(
        address: tx.address,
        flow: tx.flow == rust.PaymentFlow.Outbound ? PaymentFlow.outbound : PaymentFlow.inbound,
        amount: Amount(tx.amountSats),
        walletType: tx.walletType == rust.WalletType.Lightning ? WalletType.lightning : WalletType.onChain,
      );
    }).toList();

  static bridge.WalletInfo apiDummy() {
    return bridge.WalletInfo(
      balances: bridge.Balances(onChain: -1, lightning: -1),
      history: List.empty(growable: false),
    );
  }
}
