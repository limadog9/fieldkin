CREATE TABLE procedure_cost 
    ( 
     procedure_cost_id				INTEGER			NOT NULL , 
     procedure_occurrence_id		INTEGER			NOT NULL , 
     currency_concept_id			INTEGER			NULL ,
     paid_copay						NUMERIC			NULL , 
     paid_coinsurance				NUMERIC			NULL , 
     paid_toward_deductible			NUMERIC			NULL , 
     paid_by_payer					NUMERIC			NULL , 
     paid_by_coordination_benefits	NUMERIC			NULL , 
     total_out_of_pocket			NUMERIC			NULL , 
     total_paid						NUMERIC			NULL ,
	 revenue_code_concept_id		INTEGER			NULL ,  
     payer_plan_period_id			INTEGER			NULL ,
	 revenue_code_source_value		VARCHAR(50)		NULL
	) 
;
