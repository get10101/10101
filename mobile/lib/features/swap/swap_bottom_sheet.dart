import 'dart:async';
import 'dart:math';

import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_and_fiat_text.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/features/swap/swap_amount_text_input_form_field.dart';
import 'package:get_10101/common/application/lsp_change_notifier.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/swap/execute_swap_modal.dart';
import 'package:get_10101/features/swap/swap_trade_values.dart';
import 'package:get_10101/features/swap/swap_value_change_notifier.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

class SwapBottomSheet extends StatefulWidget {
  static const _offPurple = Color(0xfff9f9f9);

  final Position? position;

  const SwapBottomSheet({super.key, this.position});

  @override
  State<SwapBottomSheet> createState() => _StableBottomSheet();
}

class _StableBottomSheet extends State<SwapBottomSheet> {
  late final SubmitOrderChangeNotifier submitOrderChangeNotifier;

  final _formKey = GlobalKey<FormState>();

  // Which direction the trade is in. If short, we are stabilising sats to USDP
  // If long, we are bitcoinizing USDP to sats.
  Direction direction = Direction.short;

  final TextEditingController _lnController = TextEditingController();
  final TextEditingController _usdpController = TextEditingController();

  Future<(ChannelInfo?, Amount, Amount)> _getChannelInfo(
      LspChangeNotifier lspChangeNotifier) async {
    final channelInfoService = lspChangeNotifier.channelInfoService;
    var channelInfo = await channelInfoService.getChannelInfo();

    /// The max channel capacity as received by the LSP or if there is an existing channel
    var lspMaxChannelCapacity = await channelInfoService.getMaxCapacity();

    /// The max channel capacity as received by the LSP or if there is an existing channel
    Amount tradeFeeReserve = await lspChangeNotifier.getTradeFeeReserve();

    var completer = Completer<(ChannelInfo?, Amount, Amount)>();
    completer.complete((channelInfo, lspMaxChannelCapacity, tradeFeeReserve));

    return completer.future;
  }

  @override
  void initState() {
    super.initState();

    final stableValuesChangeNotifier = context.read<SwapValuesChangeNotifier>();
    final tradeValues = stableValuesChangeNotifier.stableValues();
    updateAmountFields(tradeValues);
  }

  void updateAmountFields(SwapTradeValues tradeValues) {
    _usdpController.text = tradeValues.quantity!.formatted();
    _lnController.text = tradeValues.margin!.formatted();
  }

  @override
  Widget build(BuildContext context) {
    final stableValuesChangeNotifier = context.watch<SwapValuesChangeNotifier>();
    final tradeValues = stableValuesChangeNotifier.stableValues();
    tradeValues.direction = direction;

    final LspChangeNotifier lspChangeNotifier = context.read<LspChangeNotifier>();

    WalletInfo walletInfo = context.watch<WalletChangeNotifier>().walletInfo;

    return Theme(
      data: Theme.of(context).copyWith(textTheme: GoogleFonts.interTextTheme()),
      child: Form(
          key: _formKey,
          child: Column(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              crossAxisAlignment: CrossAxisAlignment.center,
              mainAxisSize: MainAxisSize.min,
              children: [
                FutureBuilder<(ChannelInfo?, Amount, Amount)>(
                    future: _getChannelInfo(lspChangeNotifier),
                    // a previously-obtained Future<String> or null
                    builder: (BuildContext context,
                        AsyncSnapshot<(ChannelInfo?, Amount, Amount)> snapshot) {
                      if (!snapshot.hasData) {
                        return Container();
                      }

                      var (channelInfo, lspMaxChannelCapacity, tradeFeeReserve) = snapshot.data!;

                      Amount channelCapacity = lspMaxChannelCapacity;

                      Amount initialReserve =
                          lspChangeNotifier.channelInfoService.getInitialReserve();

                      Amount channelReserve = channelInfo?.reserve ?? initialReserve;
                      int totalReserve = channelReserve.sats + tradeFeeReserve.sats;

                      int usableBalance = max(walletInfo.balances.offChain.sats - totalReserve, 0);
                      // the assumed balance of the counterparty based on the channel and our balance
                      // this is needed to make sure that the counterparty can fulfil the trade
                      int counterpartyUsableBalance = max(
                          channelCapacity.sats - (walletInfo.balances.offChain.sats + totalReserve),
                          0);

                      final usdpBalQuantity = widget.position?.quantity.asDouble() ?? 0.0;

                      String? validateLn(String? value) {
                        if (value == null || value.isEmpty || value == "0") {
                          return "Enter a quantity";
                        }

                        try {
                          final margin = Amount.parseAmount(value).sats;

                          // TODO: probably should be done in backend/cached?
                          final price = Amount.fromBtc(1.0 / tradeValues.price!).sats;

                          if (margin < price) {
                            return "Min: $price";
                          }

                          if (direction == Direction.short &&
                              tradeValues.margin!.sats > usableBalance) {
                            return "Not enough funds";
                          }

                          if (direction == Direction.short && margin > counterpartyUsableBalance) {
                            return "Your counterparty does not have enough funds";
                          }
                        } catch (exception) {
                          logger.e(exception);
                          return "Enter a valid number";
                        }
                        return null;
                      }

                      String? validateUsdp(String? value) {
                        if (value == null || value.isEmpty || value == "0") {
                          return "Enter a quantity";
                        }
                        try {
                          final quantity = Amount.parseAmount(value).sats;
                          if (quantity < 1) {
                            return "The minimum quantity is 1";
                          }

                          if (direction == Direction.long && quantity > usdpBalQuantity) {
                            return "Not enough funds";
                          }
                        } catch (exception) {
                          return "Enter a valid number";
                        }
                        return null;
                      }

                      final usdpBal = FiatText(amount: usdpBalQuantity);
                      final lnBal = AmountText(amount: Amount(usableBalance));

                      final lnField = SwapAmountInputField(
                        controller: _lnController,
                        denseNoPad: true,
                        enabledColor: SwapBottomSheet._offPurple,
                        hoverColor: SwapBottomSheet._offPurple,
                        autovalidateMode: AutovalidateMode.always,
                        style: const TextStyle(
                          fontSize: 24,
                          fontWeight: FontWeight.w500,
                        ),
                        border: InputBorder.none,
                        validator: validateLn,
                        onChanged: (value) {
                          try {
                            final margin = Amount.parseAmount(value);
                            stableValuesChangeNotifier.updateMargin(margin);
                          } catch (exception) {
                            stableValuesChangeNotifier.updateMargin(Amount.zero());
                          }

                          updateAmountFields(tradeValues);
                        },
                      );

                      final usdpField = SwapAmountInputField(
                        controller: _usdpController,
                        denseNoPad: true,
                        enabledColor: SwapBottomSheet._offPurple,
                        hoverColor: SwapBottomSheet._offPurple,
                        autovalidateMode: AutovalidateMode.always,
                        style: const TextStyle(fontSize: 24, fontWeight: FontWeight.w500),
                        border: InputBorder.none,
                        onChanged: (value) {
                          try {
                            final margin = Amount.parseAmount(value);
                            stableValuesChangeNotifier.updateQuantity(margin);
                          } catch (exception) {
                            stableValuesChangeNotifier.updateQuantity(Amount.zero());
                          }

                          updateAmountFields(tradeValues);
                        },
                        validator: validateUsdp,
                      );

                      const labelStyle = TextStyle(fontSize: 20);

                      const lnLabel = Row(
                        children: [
                          Icon(BitcoinIcons.lightning),
                          Text("Lightning", style: labelStyle),
                        ],
                      );

                      const usdpLabel = Row(
                          mainAxisAlignment: MainAxisAlignment.start,
                          children: [Text("USD-P", style: labelStyle)]);

                      final (getField, getLabel, getBal, swapField, swapBal) = switch (direction) {
                        Direction.short => (usdpField, usdpLabel, usdpBal, lnField, lnBal),
                        Direction.long => (lnField, lnLabel, lnBal, usdpField, usdpBal),
                      };

                      final enabledSubmit = validateLn(_lnController.text) == null &&
                          validateUsdp(_usdpController.text) == null;

                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          const Center(
                            child: Text(
                              "Swap",
                              textAlign: TextAlign.center,
                              style: TextStyle(fontSize: 18),
                            ),
                          ),
                          const SizedBox(height: 20),
                          const SizedBox(height: 5),
                          Selector<SwapValuesChangeNotifier, Amount>(
                            selector: (_, provider) =>
                                provider.stableValues().quantity ?? Amount.zero(),
                            builder: (BuildContext context, Amount value, Widget? child) {
                              return Column(children: [
                                _SwapTile(
                                    action: "You swap",
                                    balance: swapBal,
                                    field: swapField,
                                    type: Padding(
                                        padding: const EdgeInsets.only(top: 4, left: 25),
                                        child: DropdownButton(
                                          alignment: AlignmentDirectional.centerEnd,
                                          underline: Container(),
                                          value: direction,
                                          items: const [
                                            DropdownMenuItem(
                                                value: Direction.short, child: lnLabel),
                                            DropdownMenuItem(
                                                value: Direction.long, child: usdpLabel)
                                          ],
                                          onChanged: (dir) => setState(() => direction = dir!),
                                        ))),
                                const SizedBox(height: 20),
                                _SwapTile(
                                    action: "You get",
                                    balance: getBal,
                                    type: Padding(
                                      padding: const EdgeInsets.only(top: 10.0),
                                      child: getLabel,
                                    ),
                                    field: getField),
                              ]);
                            },
                          ),
                          const SizedBox(height: 30),
                          Center(
                            child: Row(mainAxisSize: MainAxisSize.min, children: [
                              const Text("Trading at 1 BTC = ",
                                  style: TextStyle(fontWeight: FontWeight.normal)),
                              FiatText(
                                  amount: tradeValues.price!,
                                  textStyle: const TextStyle(fontWeight: FontWeight.bold)),
                            ]),
                          ),
                          const SizedBox(height: 30.0),
                          SizedBox(
                            width: MediaQuery.of(context).size.width * 0.9,
                            child: ElevatedButton(
                                onPressed: !enabledSubmit
                                    ? null
                                    : () {
                                        _showConfirmSheet(
                                            context, tradeValues, stableValuesChangeNotifier);
                                      },
                                style: ButtonStyle(
                                    padding: MaterialStateProperty.all<EdgeInsets>(
                                        const EdgeInsets.all(15)),
                                    backgroundColor: MaterialStateProperty.resolveWith((states) {
                                      if (states.contains(MaterialState.disabled)) {
                                        return tenTenOnePurple.shade100;
                                      } else {
                                        return tenTenOnePurple;
                                      }
                                    }),
                                    shape: MaterialStateProperty.resolveWith((states) {
                                      if (states.contains(MaterialState.disabled)) {
                                        return RoundedRectangleBorder(
                                          borderRadius: BorderRadius.circular(30.0),
                                          side: BorderSide(color: tenTenOnePurple.shade100),
                                        );
                                      } else {
                                        return RoundedRectangleBorder(
                                          borderRadius: BorderRadius.circular(30.0),
                                          side: const BorderSide(color: tenTenOnePurple),
                                        );
                                      }
                                    })),
                                child: const Text(
                                  "Swap",
                                  style: TextStyle(fontSize: 18, color: Colors.white),
                                )),
                          ),
                          const SizedBox(height: 20.0),
                        ],
                      );
                    })
              ])),
    );
  }
}

class _SwapTile extends StatelessWidget {
  final String action;
  final Widget balance;
  final Widget type;
  final Widget field;

  const _SwapTile(
      {required this.action, required this.balance, required this.type, required this.field});

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
          border: Border.all(width: 1, color: Colors.grey.shade300),
          color: SwapBottomSheet._offPurple,
          borderRadius: BorderRadius.circular(20)),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          children: [
            Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
              Text(action),
              _SwapBalanceText(balance: balance),
            ]),
            Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  Expanded(
                      child: Padding(
                    padding: const EdgeInsets.only(top: 16.0),
                    child: field,
                  )),
                  type,
                ]),
          ],
        ),
      ),
    );
  }
}

class _SwapBalanceText extends StatelessWidget {
  final Widget balance;

  const _SwapBalanceText({required this.balance});

  @override
  Widget build(BuildContext context) {
    return DefaultTextStyle(
      style: const TextStyle(color: Colors.grey),
      child:
          Row(mainAxisSize: MainAxisSize.max, mainAxisAlignment: MainAxisAlignment.end, children: [
        const Text("Balance: "),
        balance,
      ]),
    );
  }
}

void _showConfirmSheet(BuildContext context, SwapTradeValues tradeValues,
    SwapValuesChangeNotifier stableValuesChangeNotifier) {
  // Calculate margin based on the floored quantity
  // E.g. if 10 000 sats gives $1, and they type in 11 000, we want to show
  // margin as 10 000 sats, which is the closest dollar, as that is what the
  // trade will actually be for.
  tradeValues.updateQuantity(tradeValues.quantity!);

  final divider = [
    const SizedBox(height: 10.0),
    const Divider(),
    const SizedBox(height: 10.0),
  ];

  const labelStyle = TextStyle(fontSize: 16);
  const valueStyle = TextStyle(fontSize: 16, fontWeight: FontWeight.bold);

  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: true,
      context: context,
      builder: (BuildContext context) => Container(
            decoration: const BoxDecoration(color: Colors.white),
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    const Text("Summary", style: TextStyle(fontSize: 16)),
                    const SizedBox(height: 32.0),
                    Container(
                      decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(16),
                          color: SwapBottomSheet._offPurple),
                      child: Padding(
                        padding: const EdgeInsets.all(16),
                        child: Column(children: [
                          ValueDataRow(
                              type: tradeValues.direction == Direction.short
                                  ? ValueType.amount
                                  : ValueType.fiat,
                              value: tradeValues.direction == Direction.short
                                  ? tradeValues.margin
                                  : tradeValues.quantity!.sats.toDouble(),
                              label: "You swap:",
                              valueTextStyle: valueStyle,
                              labelTextStyle: labelStyle),
                          ...divider,
                          ValueDataRow(
                              type: tradeValues.direction == Direction.short
                                  ? ValueType.fiat
                                  : ValueType.amount,
                              value: tradeValues.direction == Direction.short
                                  ? tradeValues.quantity!.sats.toDouble()
                                  : tradeValues.margin,
                              label: "You get:",
                              valueTextStyle: valueStyle,
                              labelTextStyle: labelStyle),
                          ...divider,
                          ValueDataRow(
                              type: ValueType.widget,
                              value: AmountAndFiatText(amount: tradeValues.fee ?? Amount(0)),
                              label: "Swap fees:",
                              labelTextStyle: labelStyle),
                        ]),
                      ),
                    ),
                    const SizedBox(height: 32.0),
                    Padding(
                      padding: const EdgeInsets.only(bottom: 24.0),
                      child: ConfirmationSlider(
                          text: "Swipe to confirm",
                          textStyle: const TextStyle(color: Colors.black87),
                          height: 40,
                          foregroundColor: tenTenOnePurple,
                          sliderButtonContent: const Icon(
                            Icons.chevron_right,
                            color: Colors.white,
                            size: 20,
                          ),
                          onConfirmation: () async {
                            final submitOrderChangeNotifier =
                                context.read<SubmitOrderChangeNotifier>();

                            SwapTradeValues tradeValues = stableValuesChangeNotifier.stableValues();

                            submitOrderChangeNotifier.submitPendingOrder(
                                tradeValues.toTradeValues(), PositionAction.open,
                                stable: true);

                            // Return to the trade screen before submitting the pending order so that the dialog is displayed under the correct context
                            GoRouter.of(context).pop();
                            GoRouter.of(context).pop();

                            showExecuteSwapModal(context);
                          }),
                    )
                  ]),
            ),
          ));
}
