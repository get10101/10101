import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text_field.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/lsp_change_notifier.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/channel_opening_params.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/util/constants.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

// TODO: Fetch from backend.
Amount openingFee = Amount(0);

// TODO: Include fee reserve.

channelConfiguration({
  required BuildContext context,
  required TradeValues tradeValues,
  required Function(ChannelOpeningParams channelOpeningParams) onConfirmation,
}) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: true,
      barrierColor: Colors.black.withOpacity(0),
      context: context,
      builder: (BuildContext context) {
        return SafeArea(
            child: Padding(
                padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
                // the GestureDetector ensures that we can close the keyboard by tapping into the modal
                child: GestureDetector(
                    onTap: () {
                      FocusScopeNode currentFocus = FocusScope.of(context);

                      if (!currentFocus.hasPrimaryFocus) {
                        currentFocus.unfocus();
                      }
                    },
                    child: SingleChildScrollView(
                      child: SizedBox(
                        height: 450,
                        child: ChannelConfiguration(
                          tradeValues: tradeValues,
                          onConfirmation: onConfirmation,
                        ),
                      ),
                    ))));
      });
}

class ChannelConfiguration extends StatefulWidget {
  final TradeValues tradeValues;

  final Function(ChannelOpeningParams channelOpeningParams) onConfirmation;

  const ChannelConfiguration({super.key, required this.tradeValues, required this.onConfirmation});

  @override
  State<ChannelConfiguration> createState() => _ChannelConfiguration();
}

class _ChannelConfiguration extends State<ChannelConfiguration> {
  late final LspChangeNotifier lspChangeNotifier;

  Amount minMargin = Amount.zero();
  Amount counterpartyMargin = Amount.zero();
  Amount ownTotalCollateral = Amount.zero();
  Amount counterpartyCollateral = Amount.zero();

  double counterpartyLeverage = 1;

  Amount maxOnChainSpending = Amount.zero();
  Amount maxCounterpartyCollateral = Amount.zero();

  Amount orderMatchingFees = Amount.zero();

  final _formKey = GlobalKey<FormState>();

  @override
  void initState() {
    super.initState();

    lspChangeNotifier = context.read<LspChangeNotifier>();
    var tradeConstraints = lspChangeNotifier.channelInfoService.getTradeConstraints();

    maxCounterpartyCollateral = Amount(tradeConstraints.maxCounterpartyMarginSats);

    maxOnChainSpending = Amount(tradeConstraints.maxLocalMarginSats);
    counterpartyLeverage = tradeConstraints.coordinatorLeverage;

    counterpartyMargin = widget.tradeValues.calculateMargin(Leverage(counterpartyLeverage));

    minMargin = Amount(tradeConstraints.minMargin);

    ownTotalCollateral = tradeConstraints.minMargin > widget.tradeValues.margin!.sats
        ? Amount(tradeConstraints.minMargin)
        : widget.tradeValues.margin!;

    orderMatchingFees = widget.tradeValues.fee ?? Amount.zero();

    updateCounterpartyCollateral();

    // We add this so that the confirmation slider can be enabled immediately
    // _if_ the form is already valid. Otherwise we have to wait for the user to
    // interact with the form.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      setState(() {
        _formKey.currentState?.validate();
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    return Form(
        key: _formKey,
        child: Container(
            padding: const EdgeInsets.only(top: 20, left: 20, right: 20),
            child: Column(children: [
              const Text("DLC Channel Configuration",
                  style: TextStyle(fontWeight: FontWeight.bold, fontSize: 17)),
              const SizedBox(
                height: 20,
              ),
              RichText(
                  text: TextSpan(
                text:
                    "This is your first trade. 10101 will open a DLC channel with you, creating your position in the process.\n\nPlease specify your preferred channel size, impacting how much you will be able to win up to.",
                style: DefaultTextStyle.of(context).style,
              )),
              const SizedBox(
                height: 20,
              ),
              Center(
                child: Container(
                  padding: const EdgeInsets.symmetric(vertical: 10),
                  child: Column(
                    children: [
                      Wrap(
                        runSpacing: 10,
                        children: [
                          SizedBox(
                            height: 80,
                            child: Row(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Flexible(
                                    child: AmountInputField(
                                  value: ownTotalCollateral,
                                  label: 'Your collateral (sats)',
                                  onChanged: (value) {
                                    setState(() {
                                      ownTotalCollateral = Amount.parseAmount(value);

                                      updateCounterpartyCollateral();
                                    });
                                  },
                                  validator: (value) {
                                    if (ownTotalCollateral.sats < minMargin.sats) {
                                      return "Min collateral: $minMargin";
                                    }

                                    // TODO(holzeis): Add validation considering the on-chain fees

                                    if (ownTotalCollateral.add(orderMatchingFees).sats >
                                        maxOnChainSpending.sats) {
                                      return "Max on-chain: ${Amount(maxOnChainSpending.sats - orderMatchingFees.sats)}";
                                    }

                                    if (maxCounterpartyCollateral.sats <
                                        counterpartyCollateral.sats) {
                                      return "Over limit: $counterpartyCollateral";
                                    }

                                    return null;
                                  },
                                  infoText:
                                      "Your total collateral in the dlc channel.\n\nChoose a bigger amount here if you plan to make bigger trades in the future and don't want to open a new channel.",
                                )),
                                const SizedBox(
                                  width: 10,
                                ),
                                Flexible(
                                    child: AmountTextField(
                                  value: counterpartyCollateral,
                                  label: 'Win up to (sats)',
                                ))
                              ],
                            ),
                          ),
                          const SizedBox(height: 30),
                          ValueDataRow(
                              type: ValueType.amount,
                              value: ownTotalCollateral,
                              label: 'Your collateral'),
                          ValueDataRow(
                            type: ValueType.amount,
                            value: openingFee,
                            label: 'Channel-opening fee',
                          ),
                        ],
                      ),
                      const Divider(),
                      ValueDataRow(
                          type: ValueType.amount,
                          value: ownTotalCollateral.add(openingFee),
                          label: "Total"),
                      const SizedBox(height: 30),
                      Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          ElevatedButton(
                            key: tradeScreenBottomSheetChannelConfigurationConfirmButton,
                            onPressed:
                                _formKey.currentState != null && _formKey.currentState!.validate()
                                    ? () {
                                        GoRouter.of(context).pop();
                                        widget.onConfirmation(ChannelOpeningParams(
                                            coordinatorCollateral: counterpartyCollateral,
                                            traderCollateral: ownTotalCollateral));
                                      }
                                    : null,
                            style: ElevatedButton.styleFrom(minimumSize: const Size.fromHeight(50)),
                            child: const Text("Confirm"),
                          ),
                        ],
                      )
                    ],
                  ),
                ),
              )
            ])));
  }

  void updateCounterpartyCollateral() {
    final collateral = (ownTotalCollateral.sats / counterpartyLeverage).floor();
    counterpartyCollateral =
        counterpartyMargin.sats > collateral ? counterpartyMargin : Amount(collateral);
  }
}
