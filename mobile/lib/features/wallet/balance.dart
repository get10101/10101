import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/balance_row.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:provider/provider.dart';

class Balance extends StatelessWidget {
  const Balance({super.key});

  @override
  Widget build(BuildContext context) {
    final walletChangeNotifier = context.watch<WalletChangeNotifier>();
    Amount total = walletChangeNotifier.total();
    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();
    final position = positionChangeNotifier.positions[ContractSymbol.btcusd];
    if (position != null && position.isStable()) {
      total = total.add(position.getAmountWithUnrealizedPnl());
    }

    var (leading, balance) = getFormattedBalance(total.toInt);

    return Theme(
      data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
      child: Column(
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Text(leading,
                  style: const TextStyle(
                    color: Colors.grey,
                    fontSize: 28.0,
                    fontWeight: FontWeight.bold,
                  )),
              Text(balance,
                  style: const TextStyle(
                    color: Colors.black87,
                    fontSize: 28.0,
                    fontWeight: FontWeight.bold,
                  )),
              const Icon(Icons.currency_bitcoin, size: 28, color: tenTenOnePurple),
            ],
          ),
          const SizedBox(
            height: 20,
          ),
          _BalanceBox(),
        ],
      ),
    );
  }
}

class _BalanceBox extends StatefulWidget {
  @override
  _BalanceBoxState createState({Key? key}) => _BalanceBoxState();
}

class _BalanceBoxState extends State<_BalanceBox> {
  int selectedTitleIndex = 1;

  List<String> titles = [
    'DLC-channel',
    'On-chain',
    'USD-P',
  ];

  List<BalanceRow> balances = [
    // TODO(bonomat): this should show the balance from the dlc channel now
    const BalanceRow(walletType: WalletType.lightning),
    const BalanceRow(walletType: WalletType.onChain),
    const BalanceRow(walletType: WalletType.stable),
  ];

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: tenTenOnePurple.shade500,
        borderRadius: BorderRadius.circular(15),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(8.0),
                color: tenTenOnePurple.shade200.withOpacity(0.5),
              ),
              padding: const EdgeInsets.all(4),
              child: Row(
                children: titles
                    .map((title) => Expanded(
                          child: GestureDetector(
                            onTap: () {
                              setState(() {
                                selectedTitleIndex = titles.indexOf(title);
                              });
                            },
                            child: Container(
                              padding: const EdgeInsets.symmetric(vertical: 8),
                              alignment: Alignment.center,
                              decoration: BoxDecoration(
                                borderRadius: BorderRadius.circular(8.0),
                                color: titles.indexOf(title) == selectedTitleIndex
                                    ? tenTenOnePurple.shade900
                                    : null,
                              ),
                              child: Text(
                                title,
                                style: const TextStyle(
                                  fontWeight: FontWeight.w300,
                                  fontSize: 12,
                                  color: Colors.white,
                                ),
                              ),
                            ),
                          ),
                        ))
                    .toList(),
              )),
          const SizedBox(height: 16),
          Container(
              padding: const EdgeInsets.symmetric(vertical: 20),
              child: balances[selectedTitleIndex]),
          const SizedBox(height: 16)
        ],
      ),
    );
  }
}
