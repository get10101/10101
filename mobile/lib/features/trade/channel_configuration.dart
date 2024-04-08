import 'dart:math';

import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/dlc_channel_service.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text_field.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/tentenone_config_change_notifier.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/collateral_slider.dart';
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
      useSafeArea: true,
      builder: (BuildContext context) {
        return Padding(
          padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
          child: GestureDetector(
            onTap: () {
              FocusScopeNode currentFocus = FocusScope.of(context);

              if (!currentFocus.hasPrimaryFocus) {
                currentFocus.unfocus();
              }
            },
            child: SingleChildScrollView(
              child: ChannelConfiguration(
                tradeValues: tradeValues,
                onConfirmation: onConfirmation,
              ),
            ),
          ),
        );
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
  final TextEditingController _collateralController = TextEditingController();

  late final TenTenOneConfigChangeNotifier tentenoneConfigChangeNotifier;
  late final DlcChannelChangeNotifier dlcChannelChangeNotifier;

  Amount minMargin = Amount.zero();
  Amount counterpartyMargin = Amount.zero();
  Amount ownTotalCollateral = Amount.zero();
  Amount counterpartyCollateral = Amount.zero();

  double counterpartyLeverage = 1;

  Amount maxOnChainSpending = Amount.zero();
  Amount maxCounterpartyCollateral = Amount.zero();

  Amount orderMatchingFees = Amount.zero();

  Amount channelFeeReserve = Amount.zero();

  Amount fundingTxFee = Amount.zero();

  final _formKey = GlobalKey<FormState>();

  @override
  void initState() {
    super.initState();

    tentenoneConfigChangeNotifier = context.read<TenTenOneConfigChangeNotifier>();
    var tradeConstraints = tentenoneConfigChangeNotifier.channelInfoService.getTradeConstraints();

    DlcChannelService dlcChannelService =
        context.read<DlcChannelChangeNotifier>().dlcChannelService;

    maxCounterpartyCollateral = Amount(tradeConstraints.maxCounterpartyMarginSats);

    maxOnChainSpending = Amount(tradeConstraints.maxLocalMarginSats);
    counterpartyLeverage = tradeConstraints.coordinatorLeverage;

    counterpartyMargin = widget.tradeValues.calculateMargin(Leverage(counterpartyLeverage));

    minMargin = Amount(max(tradeConstraints.minMargin, widget.tradeValues.margin?.sats ?? 0));

    ownTotalCollateral = tradeConstraints.minMargin > widget.tradeValues.margin!.sats
        ? Amount(tradeConstraints.minMargin)
        : widget.tradeValues.margin!;

    _collateralController.text = ownTotalCollateral.formatted();

    orderMatchingFees = widget.tradeValues.fee ?? Amount.zero();

    updateCounterpartyCollateral();

    channelFeeReserve = dlcChannelService.getEstimatedChannelFeeReserve();

    fundingTxFee = dlcChannelService.getEstimatedFundingTxFee();

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
    // We add a buffer because the `fundingTxFee` is just an estimate. This
    // estimate will undershoot if we end up using more inputs or change
    // outputs.
    final Amount fundingTxFeeWithBuffer = Amount(fundingTxFee.sats * 2);
    final maxUsableOnChainBalance =
        maxOnChainSpending - orderMatchingFees - fundingTxFeeWithBuffer - channelFeeReserve;
    final maxCounterpartyCollateralSats =
        (maxCounterpartyCollateral.sats * counterpartyLeverage).toInt();

    return Form(
      key: _formKey,
      child: Container(
        padding: const EdgeInsets.only(top: 20, left: 20, right: 20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text("DLC Channel Configuration",
                    style: TextStyle(fontWeight: FontWeight.bold, fontSize: 17)),
              ],
            ),
            const SizedBox(height: 20),
            Text("This is your first trade which will open a DLC Channel and opens your position.",
                style: DefaultTextStyle.of(context).style),
            const SizedBox(height: 10),
            Text(
                "Specify your preferred channel size, impacting how much you will be able to win up to.",
                style: DefaultTextStyle.of(context).style),
            const SizedBox(height: 20),
            CollateralSlider(
              onValueChanged: (newValue) {
                setState(() {
                  ownTotalCollateral = Amount(newValue);
                  _collateralController.text = ownTotalCollateral.formatted();
                  updateCounterpartyCollateral();
                });
              },
              minValue: minMargin.sats,
              maxValue: min(maxCounterpartyCollateralSats, maxUsableOnChainBalance.toInt),
              labelText: 'Your collateral (sats)',
              value: ownTotalCollateral.sats,
            ),
            const SizedBox(height: 15),
            SizedBox(
              height: 95,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Flexible(
                    child: AmountInputField(
                      initialValue: ownTotalCollateral,
                      controller: _collateralController,
                      label: 'Your collateral (sats)',
                      onChanged: (value) {
                        setState(() {
                          ownTotalCollateral = Amount.parseAmount(value);
                          _collateralController.text = ownTotalCollateral.formatted();

                          updateCounterpartyCollateral();
                        });
                      },
                      suffixIcon: TextButton(
                        onPressed: () {
                          setState(() {
                            ownTotalCollateral = Amount(
                                min(maxCounterpartyCollateralSats, maxUsableOnChainBalance.sats));
                            _collateralController.text = ownTotalCollateral.formatted();

                            updateCounterpartyCollateral();
                          });
                        },
                        child: const Text(
                          "Max",
                          style: TextStyle(fontWeight: FontWeight.bold),
                        ),
                      ),
                      validator: (value) {
                        if (ownTotalCollateral.sats < minMargin.sats) {
                          return "Min collateral: $minMargin";
                        }

                        // TODO(holzeis): Add validation considering the on-chain fees

                        if (ownTotalCollateral.add(orderMatchingFees).sats >
                            maxOnChainSpending.sats) {
                          return "Max on-chain: $maxUsableOnChainBalance";
                        }

                        if (maxCounterpartyCollateral.sats < counterpartyCollateral.sats) {
                          return "Over limit: $maxCounterpartyCollateral";
                        }

                        return null;
                      },
                    ),
                  ),
                  const SizedBox(width: 5),
                  Flexible(
                    child: AmountTextField(
                      value: counterpartyCollateral,
                      label: 'Win up to (sats)',
                    ),
                  ),
                ],
              ),
            ),
            ValueDataRow(
                type: ValueType.amount, value: ownTotalCollateral, label: 'Your collateral'),
            ValueDataRow(
              type: ValueType.amount,
              value: openingFee,
              label: 'Channel-opening fee',
            ),
            ValueDataRow(
              type: ValueType.amount,
              value: fundingTxFee,
              label: 'Transaction fee estimate',
            ),
            ValueDataRow(
              type: ValueType.amount,
              value: channelFeeReserve,
              label: 'Channel fee reserve',
            ),
            const Divider(),
            ValueDataRow(
                type: ValueType.amount,
                value: ownTotalCollateral.add(openingFee).add(fundingTxFee).add(channelFeeReserve),
                label: "Total"),
            const SizedBox(height: 10),
            Padding(
              padding: const EdgeInsets.only(top: 8, left: 8, right: 8, bottom: 40),
              child: ElevatedButton(
                key: tradeScreenBottomSheetChannelConfigurationConfirmButton,
                onPressed: _formKey.currentState != null && _formKey.currentState!.validate()
                    ? () {
                        GoRouter.of(context).pop();
                        widget.onConfirmation(ChannelOpeningParams(
                            coordinatorCollateral: counterpartyCollateral,
                            traderCollateral: ownTotalCollateral));
                      }
                    : null,
                style: ElevatedButton.styleFrom(
                    minimumSize: const Size.fromHeight(50), backgroundColor: tenTenOnePurple),
                child: const Text(
                  "Confirm",
                  style: TextStyle(color: Colors.white),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  void updateCounterpartyCollateral() {
    final collateral = (ownTotalCollateral.sats / counterpartyLeverage).floor();
    counterpartyCollateral =
        counterpartyMargin.sats > collateral ? counterpartyMargin : Amount(collateral);
  }
}
