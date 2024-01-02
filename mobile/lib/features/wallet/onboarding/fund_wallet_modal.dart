import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/application/faucet_service.dart';
import 'package:get_10101/features/wallet/payment_claimed_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:share_plus/share_plus.dart';

void showFundWalletModal(BuildContext context, Amount amount, Amount fee, String invoice) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: false,
      context: context,
      builder: (BuildContext context) {
        return SingleChildScrollView(
            child: SizedBox(
                height: 530,
                child: Scaffold(
                  body: FundWalletModal(
                    invoice: invoice,
                    fee: fee,
                    amount: amount,
                  ),
                )));
      });
}

class FundWalletModal extends StatefulWidget {
  final String invoice;
  final Amount amount;
  final Amount fee;

  const FundWalletModal(
      {super.key, required this.invoice, required this.amount, required this.fee});

  @override
  State<FundWalletModal> createState() => _FundWalletModalState();
}

class _FundWalletModalState extends State<FundWalletModal> {
  bool _faucet = false;
  bool _isPayInvoiceButtonDisabled = false;

  @override
  void initState() {
    super.initState();
    context.read<PaymentClaimedChangeNotifier>().waitForPayment();
  }

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();
    const style = TextStyle(fontSize: 20);

    if (context.watch<PaymentClaimedChangeNotifier>().isClaimed()) {
      // We must not navigate during widget build, hence we are registering the navigation post frame.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        context
            .read<WalletChangeNotifier>()
            .refreshLightningWallet()
            .then((value) => GoRouter.of(context).go(WalletScreen.route));
      });
    }

    return SafeArea(
        child: Container(
      padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom, top: 45),
      child: Column(
        children: [
          SizedBox(
              height: 350,
              width: 260,
              child: Column(children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    const Text("Invoice",
                        style: TextStyle(fontWeight: FontWeight.bold, fontSize: 24)),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        GestureDetector(
                            onTap: () => Share.share(widget.invoice),
                            child: const Icon(Icons.share)),
                        const SizedBox(width: 15),
                        GestureDetector(
                            onTap: () {
                              Clipboard.setData(ClipboardData(text: widget.invoice)).then((_) {
                                showSnackBar(
                                    ScaffoldMessenger.of(context), "Invoice copied to clipboard");
                              });
                            },
                            child: const Icon(Icons.copy)),
                      ],
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                GestureDetector(
                  onDoubleTap: config.network == "regtest"
                      ? () {
                          setState(() {
                            _faucet = !_faucet;
                          });
                        }
                      : null,
                  child: Center(
                    child: _faucet
                        ? Column(
                            children: [
                              const SizedBox(height: 50),
                              OutlinedButton(
                                onPressed: _isPayInvoiceButtonDisabled
                                    ? null
                                    : () async {
                                        setState(() => _isPayInvoiceButtonDisabled = true);
                                        final faucetService = context.read<FaucetService>();
                                        faucetService
                                            .payInvoiceWithFaucet(widget.invoice, widget.amount)
                                            .catchError((error) {
                                          setState(() => _isPayInvoiceButtonDisabled = false);
                                          showSnackBar(
                                              ScaffoldMessenger.of(context), error.toString());
                                        });
                                      },
                                style: ElevatedButton.styleFrom(
                                  shape: const RoundedRectangleBorder(
                                      borderRadius: BorderRadius.all(Radius.circular(5.0))),
                                ),
                                child: const Text("Pay the invoice with 10101 faucet"),
                              ),
                            ],
                          )
                        : QrImageView(
                            data: widget.invoice,
                            embeddedImage:
                                const AssetImage('assets/10101_logo_icon_white_background.png'),
                            embeddedImageStyle: const QrEmbeddedImageStyle(
                              size: Size(50, 50),
                            ),
                            version: QrVersions.auto,
                            padding: const EdgeInsets.all(5),
                          ),
                  ),
                )
              ])),
          Padding(
            padding: const EdgeInsets.only(left: 30.0, right: 30.0),
            child: Column(
              children: [
                ValueDataRow(
                  type: ValueType.amount,
                  value: widget.amount,
                  label: "Amount",
                  labelTextStyle: style,
                  valueTextStyle: style,
                ),
                const Divider(),
                ValueDataRow(
                  type: ValueType.amount,
                  value: widget.fee,
                  label: "Fee",
                  labelTextStyle: style,
                  valueTextStyle: style,
                ),
                const Divider(),
              ],
            ),
          ),
        ],
      ),
    ));
  }
}
